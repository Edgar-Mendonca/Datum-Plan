# Security

## Reporting a problem

Please report a security problem privately: use **Security › Report a vulnerability** on this repository (GitHub private advisory), or open an issue that only says you have something to report and we will contact you. Do not post details in a public issue.

## What Datum Plan does and does not do

- Plans are `.dplan` files (JSON) on your own computer. They can hold people's names, rates and notes; keep them where you keep other project documents.
- The only network call is the update check and download from this repository's GitHub Releases. Every download is checked against its SHA-256 checksum before it installs.
- The window can only open links to this repository (`open_url` refuses anything else).
- Imported files (XML, XER, MPX, Excel, CSV, JSON) are treated as untrusted: the readers are tested with thousands of damaged files and must never crash the app.
- The installer is not code-signed yet (planned for 0.6), so Windows shows a SmartScreen warning on the first install.
