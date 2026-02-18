# Quick Start: Getting Audio Working

## TL;DR

Your SIP phone works perfectly! Calls connect successfully. You just need to configure audio devices on your system.

## Diagnosis

From your log:
```
✅ SIP: Working
✅ Authentication: Working  
✅ Call Connection: Working
✅ RTP Session: Working
❌ Audio Devices: Not configured
```

## Quick Fix (Choose One)

### If You're on WSL2 (Windows Subsystem for Linux):

```bash
# 1. Install PulseAudio in WSL
sudo apt-get update
sudo apt-get install pulseaudio

# 2. Download PulseAudio for Windows
# Go to: https://www.freedesktop.org/wiki/Software/PulseAudio/Ports/Windows/Support/
# Or use: https://github.com/pgaskin/pulseaudio-win32/releases

# 3. Start PulseAudio on Windows (run pulseaudio.exe)

# 4. Configure WSL to use Windows PulseAudio
echo 'export PULSE_SERVER=tcp:$(grep nameserver /etc/resolv.conf | awk "{print \$2}")' >> ~/.bashrc
source ~/.bashrc

# 5. Test
pactl list sinks short
```

### If You're on Physical Linux:

```bash
# 1. Install ALSA utilities
sudo apt-get update
sudo apt-get install alsa-utils

# 2. Check for audio devices
aplay -l    # Should show playback devices
arecord -l  # Should show capture devices

# 3. If no devices, check if sound card is detected
lspci | grep -i audio
lsusb | grep -i audio

# 4. Test speaker
speaker-test -t wav -c 2

# 5. Test microphone
arecord -d 5 test.wav && aplay test.wav
```

### If You're on Docker:

```bash
# Run with audio device passthrough
docker run --device /dev/snd:/dev/snd \
           -v /run/user/$(id -u)/pulse:/run/user/$(id -u)/pulse \
           your-image
```

### If You Have a USB Headset:

```bash
# 1. Plug in USB headset

# 2. Check if detected
aplay -l
arecord -l

# 3. Should see something like:
# card 1: Headset [USB Headset], device 0: USB Audio [USB Audio]

# 4. Test
speaker-test -D hw:1,0 -t wav -c 2
arecord -D hw:1,0 -d 5 test.wav && aplay test.wav
```

## Verify Audio is Working

```bash
# List playback devices
aplay -l

# Expected output:
# **** List of PLAYBACK Hardware Devices ****
# card 0: PCH [HDA Intel PCH], device 0: ALC269VC Analog [ALC269VC Analog]
#   Subdevices: 1/1
#   Subdevice #0: subdevice #0

# List capture devices  
arecord -l

# Expected output:
# **** List of CAPTURE Hardware Devices ****
# card 0: PCH [HDA Intel PCH], device 0: ALC269VC Analog [ALC269VC Analog]
#   Subdevices: 1/1
#   Subdevice #0: subdevice #0
```

## Run Your App

Once audio devices are configured:

```bash
cd /home/diesel/Projects/platypus-phone
npm run tauri dev
```

Expected log output:
```
[Audio] Initializing audio devices...
[Audio] Available audio host: ALSA
[Audio] Using input device: HDA Intel PCH
[Audio] Input config: SupportedStreamConfig { channels: 1, sample_rate: SampleRate(8000), buffer_size: Fixed(160), sample_format: I16 }
[Audio] ✓ Microphone capture started
[Audio] Using output device: HDA Intel PCH
[Audio] Output config: SupportedStreamConfig { channels: 1, sample_rate: SampleRate(8000), buffer_size: Fixed(160), sample_format: I16 }
[Audio] ✓ Speaker playback started
[RTP] ✓✓✓ RTP media session active! ✓✓✓
[SIP] ✓ RTP media active - call has audio!
```

## Test the Call

1. **Register** with your SIP server
2. **Make a call** to another extension
3. **You should hear** the remote party
4. **Remote party should hear** you
5. **Check logs** for RTP packet counts:
   ```
   [RTP] Sent 50 packets
   [RTP] Received 50 packets
   ```

## Troubleshooting

### Still No Audio Devices?

```bash
# Check if ALSA modules are loaded
lsmod | grep snd

# If not, load them
sudo modprobe snd-hda-intel

# Check PulseAudio status
systemctl --user status pulseaudio

# Restart PulseAudio
systemctl --user restart pulseaudio
```

### Audio Devices Exist But App Can't Find Them?

```bash
# Check permissions
groups $USER

# Should include 'audio' group
# If not, add yourself:
sudo usermod -a -G audio $USER

# Log out and back in for changes to take effect
```

### Using PipeWire Instead of PulseAudio?

```bash
# PipeWire is compatible with PulseAudio
# Just make sure pipewire-pulse is running
systemctl --user status pipewire-pulse

# If not running:
systemctl --user start pipewire-pulse
```

## Alternative: Test with Dummy Devices (Development Only)

If you just want to test the SIP/RTP functionality without real audio:

```bash
# Load ALSA dummy driver
sudo modprobe snd-dummy

# Verify
aplay -l
# Should show: card 0: Dummy [Dummy], device 0: Dummy PCM [Dummy PCM]

# Now run your app
# It will "work" but you won't hear anything (dummy device)
```

## Need Help?

Check these files for more details:
- `AUDIO_ISSUE_ANALYSIS.md` - Detailed analysis
- `COMPLETE_SUMMARY.md` - Full implementation summary
- `FIXES_IMPLEMENTED.md` - SIP authentication fixes

## What's Next?

Once audio is working, consider adding:
1. Settings UI for device selection
2. Volume controls
3. Echo cancellation
4. Noise suppression
5. Incoming call support

But for now, just get audio devices configured and your phone will work perfectly!
