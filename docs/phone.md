# Phone companion

Phone access is off until you turn it on. Then OpenCompanion serves the phone page over HTTP and WebSocket on your local network, on port 8765 unless you pick another one in Settings.

1. On the desktop, open Settings › Phone access and press Turn on phone access. If Windows Firewall asks, allow OpenCompanion on private networks; if macOS asks whether to accept incoming connections, allow them.
2. On the phone, join the same Wi-Fi and scan the QR code with the camera, or open the address shown and type the 6-digit pairing code. A code works once, expires after 2 minutes and allows 5 wrong tries.
3. Give the phone a name. It then shows your sessions.

Settings › Phone access lists the paired devices. Remove one and it has to pair again. Turning phone access off disconnects every phone.

Good to know:

- The connection is plain HTTP. On a network you do not trust, or away from home, reach your computer through a VPN with HTTPS, such as Tailscale.
- The page has a web app manifest and Apple tags, so you can add it to the home screen. On the plain-HTTP LAN, Chrome shows no install prompt and no service worker runs, so the installed app opens only while the desktop answers. Over HTTPS or on localhost, the service worker keeps the app shell and the offline screen.
- On iPhone, the Home Screen app keeps its own storage, apart from Safari, so pair it once more by typing the code.
- The phone shows notifications only while its page is open. Web Push is not built yet.
