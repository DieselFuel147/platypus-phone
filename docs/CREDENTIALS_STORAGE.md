# Credentials Storage

## Overview

Platypus Phone now automatically saves your SIP credentials (server, username, and password) between sessions. When you close and reopen the app, your credentials will be automatically loaded and filled in.

## How It Works

### Storage Location

Credentials are stored in a JSON file in your application data directory:

- **Windows**: `%APPDATA%\platypus-phone\settings.json`
- **Linux**: `~/.local/share/platypus-phone/settings.json`
- **macOS**: `~/Library/Application Support/platypus-phone/settings.json`

### Security

The password is **obfuscated** using a simple XOR-based encryption with a fixed key. This provides basic protection against casual viewing of the password in the settings file.

**Important Security Notes:**
- This is NOT cryptographically secure encryption
- It prevents casual viewing but won't stop a determined attacker
- The password is obfuscated, not encrypted with a user-specific key
- This is suitable for testing and development environments
- For production use, consider implementing proper encryption with OS keychain integration

### What Gets Saved

When you successfully register with a SIP server, the following information is saved:
- SIP Server address
- Username
- Password (obfuscated)

### Automatic Loading

When you start the app:
1. The app checks for saved credentials
2. If found, the credentials are loaded and the input fields are pre-filled
3. You can then click "Register" to connect with the saved credentials
4. If no saved credentials exist, the fields remain empty

### Manual Management

The credentials are automatically saved when you successfully register. If you want to:

- **Update credentials**: Simply enter new credentials and register again
- **Clear credentials**: Delete the `settings.json` file from the app data directory

## Implementation Details

### Backend (Rust)

The credentials storage is implemented in `src-tauri/src/settings.rs`:

- `save_credentials()` - Saves credentials to disk with password obfuscation
- `load_credentials()` - Loads credentials from disk and deobfuscates password
- `clear_credentials()` - Deletes the saved credentials file

### Frontend (TypeScript)

The frontend automatically:
- Loads credentials on app startup (in `useEffect`)
- Saves credentials after successful registration (in `handleRegister`)

### Tauri Commands

Three new Tauri commands are available:
- `save_sip_credentials` - Save credentials
- `load_sip_credentials` - Load credentials
- `clear_sip_credentials` - Clear saved credentials

## Works with Built .exe

Yes! This feature works perfectly with the built `.exe` file. The settings file will be created in the appropriate Windows AppData directory when you run the built executable.

To build the Windows executable:
```bash
npm run tauri build
```

The `.exe` will be located in:
```
src-tauri/target/release/platypus-phone.exe
```

When you run this executable, it will:
1. Create the settings directory if it doesn't exist
2. Save credentials when you register
3. Load credentials on subsequent launches

## Future Improvements

For production use, consider:
- Integration with OS keychain/credential manager (Windows Credential Manager, macOS Keychain, Linux Secret Service)
- User-specific encryption keys
- Option to disable credential storage
- Master password protection
- Credential expiration/rotation
