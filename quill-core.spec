# -*- mode: python ; coding: utf-8 -*-


a = Analysis(
    ['quill_entry.py'],
    pathex=[],
    binaries=[],
    datas=[('config/default.yaml', 'config'), ('config/modes.yaml', 'config')],
    hiddenimports=['core', 'core.main', 'core.config_loader', 'core.streamer', 'core.history', 'core.tutor', 'core.clipboard_monitor', 'core.platform', 'core.prompt_builder', 'providers', 'providers.openrouter', 'providers.ollama', 'providers.openai', 'providers.generic', 'providers.generic_endpoint', 'platform_'],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[],
    noarchive=False,
    optimize=0,
)
pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name='quill-core',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=True,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)
