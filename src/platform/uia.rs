//! `IUIAutomation` wrapper: focused element + selected text + bounds.
//!
//! Thread-local singleton — COM is initialized once per thread on first access.
//! All methods are synchronous; callers that need async must spawn a blocking
//! task.

use std::cell::RefCell;

use anyhow::{anyhow, Context, Result};
use windows::Win32::Foundation::{BOOL, RECT, RPC_E_CHANGED_MODE};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::System::Ole::{
    SafeArrayAccessData, SafeArrayDestroy, SafeArrayGetLBound, SafeArrayGetUBound,
    SafeArrayGetVartype, SafeArrayUnaccessData,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern,
    IUIAutomationTextRange, UIA_DocumentControlTypeId, UIA_EditControlTypeId, UIA_TextPatternId,
};

use super::traits::ScreenRect;

pub struct Uia {
    automation: IUIAutomation,
}

thread_local! {
    static UIA_TLS: RefCell<Option<Uia>> = const { RefCell::new(None) };
}

impl Uia {
    /// Access a thread-local UIA instance.  Lazily initialises COM on first call.
    pub fn with<R>(f: impl FnOnce(&Uia) -> R) -> Result<R> {
        UIA_TLS.with(|cell| {
            let mut borrow = cell.borrow_mut();
            if borrow.is_none() {
                *borrow = Some(Uia::new()?);
            }
            Ok(f(borrow.as_ref().unwrap()))
        })
    }

    fn new() -> Result<Self> {
        unsafe {
            let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
            // S_OK (0) and S_FALSE (1) are both success (S_FALSE = already init on thread).
            // RPC_E_CHANGED_MODE means another apartment type was already set — still usable.
            if hr.is_err() && hr != RPC_E_CHANGED_MODE {
                return Err(anyhow!("CoInitializeEx failed: 0x{:08X}", hr.0));
            }
        }

        let automation: IUIAutomation = unsafe {
            CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
                .context("CoCreateInstance(CUIAutomation)")?
        };

        Ok(Self { automation })
    }

    /// Return the currently focused UI element.
    pub fn focused_element(&self) -> Result<IUIAutomationElement> {
        unsafe {
            self.automation
                .GetFocusedElement()
                .context("GetFocusedElement")
        }
    }

    /// Return the selected text from the focused element, or `None` if the
    /// element doesn't support the Text pattern or nothing is selected.
    pub fn selected_text(&self) -> Result<Option<String>> {
        let element = self.focused_element()?;
        let Some(pattern) = get_text_pattern(&element)? else {
            return Ok(None);
        };

        let ranges = unsafe { pattern.GetSelection().context("GetSelection")? };
        let count = unsafe { ranges.Length().context("ranges.Length")? };
        if count == 0 {
            return Ok(None);
        }

        let mut combined = String::new();
        for i in 0..count {
            let range: IUIAutomationTextRange =
                unsafe { ranges.GetElement(i).context("ranges.GetElement")? };
            let bstr = unsafe { range.GetText(-1).context("range.GetText")? };
            let s = bstr.to_string();
            // Rich-text editors expose a single contiguous selection as
            // multiple ranges (one per run / style change). Inserting a
            // space between them corrupts the captured text — e.g. a
            // bolded word in the middle of a sentence would come back as
            // "Hello bold world" instead of "Hello bold world" with no
            // extra whitespace. Concatenate with empty string.
            combined.push_str(&s);
        }

        let trimmed = combined.trim().to_owned();
        if trimmed.is_empty() {
            Ok(None)
        } else {
            Ok(Some(trimmed))
        }
    }

    /// Return the bounding rectangle of the current text selection (union of
    /// all selected ranges), or `None` if there is no selection or the element
    /// doesn't support the Text pattern.
    pub fn selection_bounds(&self) -> Result<Option<ScreenRect>> {
        let element = self.focused_element()?;
        let Some(pattern) = get_text_pattern(&element)? else {
            return Ok(None);
        };

        let ranges = unsafe { pattern.GetSelection().context("GetSelection")? };
        let count = unsafe { ranges.Length().context("ranges.Length")? };
        if count == 0 {
            return Ok(None);
        }

        // Iterate over EVERY selected range and union all their rects.
        // Earlier code only looked at `GetElement(0)` which missed
        // multi-range selections (supported by controls that allow
        // disjoint selections like rich-text editors).
        let mut union = ScreenRect {
            left: i32::MAX,
            top: i32::MAX,
            right: i32::MIN,
            bottom: i32::MIN,
        };
        let mut any_rect = false;
        for i in 0..count {
            let range: IUIAutomationTextRange =
                unsafe { ranges.GetElement(i).context("ranges.GetElement")? };
            let rects_safearray = unsafe {
                range
                    .GetBoundingRectangles()
                    .context("GetBoundingRectangles")?
            };
            // SAFETY: rects_safearray is a raw SAFEARRAY pointer returned by Windows.
            let rects = unsafe { safearray_to_f64_vec(rects_safearray)? };
            // Each entry in the flat array is a (left, top, width, height) quad.
            for chunk in rects.chunks_exact(4) {
                let l = chunk[0] as i32;
                let t = chunk[1] as i32;
                let w = chunk[2] as i32;
                let h = chunk[3] as i32;
                union.left = union.left.min(l);
                union.top = union.top.min(t);
                union.right = union.right.max(l + w);
                union.bottom = union.bottom.max(t + h);
                any_rect = true;
            }
        }
        if !any_rect {
            return Ok(None);
        }
        Ok(Some(union))
    }

    /// Bounding rectangle of the currently focused UI element as a whole —
    /// e.g. the entire text area in a document editor. NOT the caret itself.
    ///
    /// Used as a fallback anchor for near-caret overlay positioning when
    /// `selection_bounds()` returns `None` (focus in a non-text control,
    /// empty selection, or Text pattern unsupported).
    pub fn element_bounds(&self) -> Result<Option<ScreenRect>> {
        let element = self.focused_element()?;
        let rect: RECT = unsafe {
            element
                .CurrentBoundingRectangle()
                .context("CurrentBoundingRectangle")?
        };
        Ok(Some(ScreenRect {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        }))
    }

    /// Combined lookup for the pencil-controller hot path: fetch the
    /// focused element ONCE, check editability on it, and return its
    /// bounding rect + editability. Reduces the 3 round-trip COM call
    /// pattern (focused_element → focused_element in is_editable_text →
    /// focused_element in element_bounds) to a single `focused_element`.
    ///
    /// Returns `Ok(Some((rect, true)))` when the focused element is an
    /// editable text control, `Ok(Some((rect, false)))` when it's not
    /// (so the caller can hide the pencil), or `Ok(None)` if no element
    /// is focused.
    pub fn editable_element_bounds(&self) -> Result<Option<(ScreenRect, bool)>> {
        let element = match self.focused_element() {
            Ok(e) => e,
            Err(_) => return Ok(None),
        };
        let editable = self.is_editable_text(&element).unwrap_or(false);
        let rect: RECT = unsafe {
            element
                .CurrentBoundingRectangle()
                .context("CurrentBoundingRectangle")?
        };
        Ok(Some((
            ScreenRect {
                left: rect.left,
                top: rect.top,
                right: rect.right,
                bottom: rect.bottom,
            },
            editable,
        )))
    }

    /// Return `true` if `element` is an editable text control (Edit or
    /// Document), is enabled, and supports the UIA Text pattern.
    pub fn is_editable_text(&self, element: &IUIAutomationElement) -> Result<bool> {
        let control_type = unsafe { element.CurrentControlType().context("CurrentControlType")? };
        let is_edit_or_doc =
            control_type == UIA_EditControlTypeId || control_type == UIA_DocumentControlTypeId;
        // CurrentName() is a cross-process COM call with measurable
        // latency; it fires on EVERY focus event today. Gate the whole
        // diagnostic block on TRACE — release and debug-INFO paths skip
        // the call entirely.
        if tracing::enabled!(tracing::Level::TRACE) {
            let name = unsafe {
                element
                    .CurrentName()
                    .ok()
                    .map(|n| n.to_string())
                    .unwrap_or_default()
            };
            tracing::trace!(
                control_type = control_type.0,
                is_edit_or_doc,
                name = %name,
                "is_editable_text: inspecting element"
            );
        }
        if !is_edit_or_doc {
            return Ok(false);
        }
        let enabled: BOOL = unsafe { element.CurrentIsEnabled().context("CurrentIsEnabled")? };
        if !enabled.as_bool() {
            return Ok(false);
        }
        let pattern = get_text_pattern(element)?;
        Ok(pattern.is_some())
    }
}

/// Try to get the `IUIAutomationTextPattern` from `element`.
///
/// Returns `Ok(None)` if the element does not support the Text pattern (the
/// COM query returns `E_NOINTERFACE` or the element doesn't expose it).
fn get_text_pattern(element: &IUIAutomationElement) -> Result<Option<IUIAutomationTextPattern>> {
    // GetCurrentPatternAs performs QueryInterface directly; it returns an error
    // (typically E_NOINTERFACE or UIA_E_NOTSUPPORTED) when the pattern is
    // absent.  We treat that as Ok(None) — not an error worth propagating.
    match unsafe { element.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) } {
        Ok(pattern) => Ok(Some(pattern)),
        Err(e) => {
            use windows::Win32::Foundation::E_NOINTERFACE;
            // If the element simply doesn't implement the pattern, return None.
            // For any other error, propagate it so the caller knows something
            // unexpected happened.
            if e.code() == E_NOINTERFACE {
                Ok(None)
            } else {
                // Some hosts return S_OK with a null pointer when the pattern is
                // unsupported — but GetCurrentPatternAs will fail to from_abi a
                // null, producing a different HRESULT.  Treat any failure as
                // "pattern not available" since the UIA spec permits multiple
                // success/failure paths for absent patterns.
                tracing::debug!("GetCurrentPatternAs failed ({e}); treating as no text pattern");
                Ok(None)
            }
        }
    }
}

/// RAII wrapper that guarantees a SAFEARRAY returned by UIA is destroyed
/// exactly once — even when `to_vec()` panics mid-copy (OOM, etc.).
///
/// Without this, the earlier straight-line version would leak the array
/// on any panic between `SafeArrayAccessData` and `SafeArrayDestroy`.
struct SafeArrayOwned {
    array: *mut windows::Win32::System::Com::SAFEARRAY,
    locked: bool,
}

impl Drop for SafeArrayOwned {
    fn drop(&mut self) {
        unsafe {
            if self.locked {
                SafeArrayUnaccessData(self.array).ok();
            }
            SafeArrayDestroy(self.array).ok();
        }
    }
}

/// Copy the `f64` values out of a `SAFEARRAY` returned by UIA.
///
/// # Safety
///
/// `array` must be a valid pointer to a `SAFEARRAY` of `VT_R8` (f64) as
/// produced by `IUIAutomationTextRange::GetBoundingRectangles`, or null.
unsafe fn safearray_to_f64_vec(
    array: *mut windows::Win32::System::Com::SAFEARRAY,
) -> Result<Vec<f64>> {
    if array.is_null() {
        return Ok(Vec::new());
    }

    // Take ownership immediately so the array is destroyed on any exit
    // path, including a panic inside `slice.to_vec()`.
    let mut owned = SafeArrayOwned {
        array,
        locked: false,
    };

    // Verify the SAFEARRAY really contains VT_R8 (f64) values before we
    // reinterpret its buffer as `*const f64`. UIA should always produce
    // VT_R8 for GetBoundingRectangles, but a driver bug or future API
    // change handing us e.g. VT_I4 would silently produce garbage
    // rectangles if we skipped this check. VT_R8 == 5.
    let vt = SafeArrayGetVartype(owned.array).context("SafeArrayGetVartype")?;
    if vt.0 != 5 {
        return Ok(Vec::new());
    }

    let lbound = SafeArrayGetLBound(owned.array, 1).context("SafeArrayGetLBound")?;
    let ubound = SafeArrayGetUBound(owned.array, 1).context("SafeArrayGetUBound")?;
    // An empty SAFEARRAY reports ubound < lbound (e.g. lbound=0, ubound=-1).
    // Without this guard the `(ubound - lbound + 1)` subtraction below would
    // either underflow the i32 arithmetic or cast to a huge `usize`,
    // producing a bogus `from_raw_parts` with a giant length and a segfault.
    if ubound < lbound {
        return Ok(Vec::new());
    }
    let len = (ubound - lbound + 1) as usize;

    if len == 0 {
        return Ok(Vec::new());
    }

    let mut data_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
    SafeArrayAccessData(owned.array, &mut data_ptr).context("SafeArrayAccessData")?;
    owned.locked = true;
    // Defensive null check — SafeArrayAccessData *should* always give us a
    // valid pointer after returning Ok, but if a driver ever hands back a
    // null `data_ptr` without failing the HRESULT, `from_raw_parts` on a
    // null base with non-zero len is instant UB. Better to return empty.
    if data_ptr.is_null() {
        return Ok(Vec::new());
    }
    let slice = std::slice::from_raw_parts(data_ptr as *const f64, len);
    let vec = slice.to_vec();
    // Let `SafeArrayOwned::drop` unlock + destroy — single code path for
    // both the happy case and any panic/early-return above. Calling
    // SafeArrayUnaccessData eagerly here would just duplicate that logic
    // and risk a stale `locked` flag if the explicit unlock silently
    // failed.
    Ok(vec)
}
