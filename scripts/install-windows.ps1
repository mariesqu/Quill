# Quill — Windows one-liner installer
# Usage: powershell -ExecutionPolicy Bypass -File scripts\install-windows.ps1

$ErrorActionPreference = "Stop"

$QuillDir = "$env:USERPROFILE\.quill"
$Repo = "https://github.com/mariesqu/Quill"

Write-Host ""
Write-Host "🪶 Quill — Windows Installer" -ForegroundColor Cyan
Write-Host "==================================" -ForegroundColor Cyan

# ── Python check ──────────────────────────────────────────────────────────────
if (-not (Get-Command python -ErrorAction SilentlyContinue)) {
    Write-Host "→ Python not found. Installing via winget..." -ForegroundColor Yellow
    winget install Python.Python.3.11 --silent
    # Refresh PATH
    $env:PATH = [System.Environment]::GetEnvironmentVariable("PATH", "Machine") + ";" + [System.Environment]::GetEnvironmentVariable("PATH", "User")

    if (-not (Get-Command python -ErrorAction SilentlyContinue)) {
        Write-Host "❌ Python installation failed. Please install manually from python.org" -ForegroundColor Red
        exit 1
    }
}

# ── Create venv ───────────────────────────────────────────────────────────────
Write-Host "→ Setting up Python environment..."
New-Item -ItemType Directory -Force -Path $QuillDir | Out-Null
python -m venv "$QuillDir\venv"
& "$QuillDir\venv\Scripts\Activate.ps1"

# ── Python deps ───────────────────────────────────────────────────────────────
Write-Host "→ Installing Python dependencies..."
pip install --upgrade pip -q
pip install httpx pyyaml pyperclip pynput keyboard pywinauto pygetwindow psutil -q

# ── Quill source ──────────────────────────────────────────────────────────────
if (Test-Path "$QuillDir\src") {
    Write-Host "→ Updating Quill source..."
    git -C "$QuillDir\src" pull
} else {
    Write-Host "→ Cloning Quill..."
    git clone $Repo "$QuillDir\src"
}

# ── Launcher batch file ───────────────────────────────────────────────────────
Write-Host "→ Creating launcher..."
$LauncherContent = @"
@echo off
call "$QuillDir\venv\Scripts\activate.bat"
cd /d "$QuillDir\src"
python -m core.main %*
"@
$LauncherContent | Set-Content -Path "$QuillDir\quill.bat" -Encoding ASCII

# Add to PATH if not already there
$UserPath = [System.Environment]::GetEnvironmentVariable("PATH", "User")
if ($UserPath -notlike "*$QuillDir*") {
    [System.Environment]::SetEnvironmentVariable("PATH", "$UserPath;$QuillDir", "User")
}

Write-Host ""
Write-Host "✅ Quill installed! Restart your terminal then run: quill" -ForegroundColor Green
Write-Host "   Hotkey: Ctrl+Shift+Space" -ForegroundColor Green
