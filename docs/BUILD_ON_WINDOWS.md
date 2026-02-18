# Building Platypus Phone on Windows (Native)

## The Problem

When you run `npm run tauri build` in WSL, it builds for **Linux** (AppImage, deb, rpm), not Windows.

To get a Windows .exe, you need to build **on Windows** or cross-compile (which is complex).

## Solution: Build on Windows

### Prerequisites (Install on Windows)

1. **Node.js** - https://nodejs.org/ (LTS version)
2. **Rust** - https://rustup.rs/
   - Download and run `rustup-init.exe`
   - Choose default installation
3. **Visual Studio Build Tools** - Required for Rust on Windows
   - Download: https://visualstudio.microsoft.com/downloads/
   - Install "Desktop development with C++"
   - Or use: https://aka.ms/vs/17/release/vs_BuildTools.exe

### Step-by-Step Build Process

#### 1. Open PowerShell or Command Prompt on Windows

```powershell
# Navigate to your project (adjust path as needed)
cd C:\Users\YourUsername\Projects\platypus-phone

# Or if accessing from WSL path:
cd \\wsl$\Ubuntu\home\diesel\Projects\platypus-phone
```

#### 2. Install Dependencies

```powershell
# Install npm dependencies
npm install

# Verify Rust is installed
rustc --version
cargo --version
```

#### 3. Build for Windows

```powershell
# Build the Windows executable
npm run tauri build
```

This will create:
- `src-tauri\target\release\platypus-phone.exe`
- `src-tauri\target\release\bundle\msi\platypus-phone_0.1.0_x64_en-US.msi`

#### 4. Run the Executable

```powershell
# Run directly
.\src-tauri\target\release\platypus-phone.exe

# Or double-click it in File Explorer
```

### Build Output Locations

```
src-tauri\target\release\
├── platypus-phone.exe          ← Main executable
└── bundle\
    ├── msi\
    │   └── platypus-phone_0.1.0_x64_en-US.msi  ← Installer
    └── nsis\
        └── platypus-phone_0.1.0_x64-setup.exe  ← Alternative installer
```

## Alternative: Copy Project to Windows

If you don't want to install everything on Windows, you can:

### 1. Copy Project to Windows

From WSL:
```bash
# Copy entire project to Windows
cp -r /home/diesel/Projects/platypus-phone /mnt/c/Users/$USER/Desktop/
```

### 2. Build on Windows

Open PowerShell on Windows:
```powershell
cd C:\Users\YourUsername\Desktop\platypus-phone
npm install
npm run tauri build
```

## Quick Setup Script for Windows

Save this as `setup-windows.ps1`:

```powershell
# Check prerequisites
Write-Host "Checking prerequisites..." -ForegroundColor Cyan

# Check Node.js
if (Get-Command node -ErrorAction SilentlyContinue) {
    Write-Host "✓ Node.js installed: $(node --version)" -ForegroundColor Green
} else {
    Write-Host "✗ Node.js not found. Install from https://nodejs.org/" -ForegroundColor Red
    exit 1
}

# Check Rust
if (Get-Command cargo -ErrorAction SilentlyContinue) {
    Write-Host "✓ Rust installed: $(rustc --version)" -ForegroundColor Green
} else {
    Write-Host "✗ Rust not found. Install from https://rustup.rs/" -ForegroundColor Red
    exit 1
}

# Install dependencies
Write-Host "`nInstalling dependencies..." -ForegroundColor Cyan
npm install

# Build
Write-Host "`nBuilding for Windows..." -ForegroundColor Cyan
npm run tauri build

if ($LASTEXITCODE -eq 0) {
    Write-Host "`n✓ Build successful!" -ForegroundColor Green
    Write-Host "Executable: src-tauri\target\release\platypus-phone.exe" -ForegroundColor Yellow
    Write-Host "Installer: src-tauri\target\release\bundle\msi\*.msi" -ForegroundColor Yellow
} else {
    Write-Host "`n✗ Build failed!" -ForegroundColor Red
    exit 1
}
```

Run it:
```powershell
.\setup-windows.ps1
```

## Troubleshooting

### "cargo: command not found"

Rust isn't installed or not in PATH. Install from https://rustup.rs/

### "link.exe not found"

Visual Studio Build Tools not installed. Install from:
https://visualstudio.microsoft.com/downloads/

### "npm: command not found"

Node.js not installed. Install from https://nodejs.org/

### Build is slow

First build takes 5-10 minutes. Subsequent builds are faster (2-3 minutes).

### Windows Defender blocks the .exe

This is normal for unsigned executables:
1. Click "More info"
2. Click "Run anyway"

Or add an exception in Windows Defender.

## Development Workflow

### Option A: Develop in WSL, Build on Windows
```bash
# In WSL - Fast development
npm run tauri dev

# On Windows - Test with audio
npm run tauri build
```

### Option B: Everything on Windows
```powershell
# On Windows - Development
npm run tauri dev

# On Windows - Build
npm run tauri build
```

## Cross-Compilation (Advanced)

If you really want to build for Windows from WSL, you need to:

1. Install Windows target:
```bash
rustup target add x86_64-pc-windows-gnu
```

2. Install MinGW:
```bash
sudo apt-get install mingw-w64
```

3. Configure Cargo:
```bash
mkdir -p ~/.cargo
cat >> ~/.cargo/config.toml << EOF
[target.x86_64-pc-windows-gnu]
linker = "x86_64-w64-mingw32-gcc"
ar = "x86_64-w64-mingw32-ar"
EOF
```

4. Build:
```bash
cargo build --release --target x86_64-pc-windows-gnu
```

**Note**: Cross-compilation is complex and may have issues. Building natively on Windows is much easier and more reliable.

## Summary

**Easiest Method**: 
1. Copy project to Windows
2. Install Node.js and Rust on Windows
3. Run `npm run tauri build` on Windows
4. Get your .exe!

**Result**: 
- `platypus-phone.exe` that runs natively on Windows
- Full audio support with Windows audio devices
- No WSL limitations
