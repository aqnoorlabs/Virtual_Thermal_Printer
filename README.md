# 🖨 AqNoor Virtual Thermal Printer

> **High-Performance ESC/POS Thermal Receipt Printer Emulator & Live Previewer**  
> Built with **Tauri v2**, **Rust**, and **Modern Vanilla Web UI**.

---

## 📑 Table of Contents
- [🌟 Features](#-features)
- [📋 Prerequisites](#-prerequisites)
- [🚀 App Installation & Setup](#-app-installation--setup)
- [🖨️ Installing as a Windows Printer](#️-installing-as-a-windows-printer)
  - [Method 1: One-Click PowerShell Setup (Recommended)](#method-1-one-click-powershell-setup-recommended)
  - [Method 2: Windows GUI Setup](#method-2-windows-gui-setup)
- [🧪 Testing Your Printer](#-testing-your-printer)
  - [1. Built-in App Test Tools](#1-built-in-app-test-tools)
  - [2. Windows Test Print Page](#2-windows-test-print-page)
  - [3. Testing with PowerShell (Quick Script)](#3-testing-with-powershell-quick-script)
  - [4. Testing with Node.js / JavaScript](#4-testing-with-nodejs--javascript)
  - [5. Testing with Python](#5-testing-with-python)
  - [6. Testing with POS Apps (e.g. PharmaPOS)](#6-testing-with-pos-apps-eg-pharmapos)
- [📱 Supported QR & Barcode Formats](#-supported-qr--barcode-formats)
- [🛠 Project Architecture](#-project-architecture)
- [🧪 Automated Unit Tests](#-automated-unit-tests)
- [📦 Production Build](#-production-build)
- [❓ Troubleshooting & FAQ](#-troubleshooting--faq)

---

## 🌟 Features

- ⚡ **TCP JetDirect / RAW Printer Server**: Listens on `127.0.0.1:9100` (or any custom port) for direct socket connections from POS apps, Windows Print Spooler, or mobile/desktop billing software.
- 🧾 **Pixel-Perfect ESC/POS Receipt Preview**: Real-time rendering of 80mm and 58mm thermal receipts with font scaling, bold, underlines, alignments, double height/width, and paper cuts.
- 📱 **Complete QR Code Support**:
  - **Epson Standard `GS ( k`** (Model, Module Size, ECC L/M/Q/H, Store & Print)
  - **Raster Image QR `GS v 0`** (Direct bitmap image streams used by POS drivers like PharmaPOS)
  - **2D Barcode `ESC Z`** (Common in Chinese / Xprinter / POS-58 / POS-80 devices)
  - **Barcode QR `GS k 104` / `GS k 97` / `GS k 32`**
  - **Embedded Text Tags** (`[UPI_QR:...]`, `[QR:...]`, `[QR_CODE:...]`)
- 📊 **1D Barcode Rendering**: CODE128, CODE39, EAN-13, EAN-8, UPC-A, UPC-E, ITF, Codabar, Code93, and PDF417 with HRI text positioning.
- 🔍 **Live Inspector & Debug Tools**:
  - Raw Hex Dump with byte counter
  - Command-by-command opcode disassembler
  - Real-time parser warnings & connection logs
  - One-click sample test receipts (Standard QR + Barcodes, UPI Payment QR)
  - Raw Hex Byte injector tool
  - Save receipt as self-contained HTML

---

## 📋 Prerequisites

Make sure you have the following installed on your machine:

1. **Node.js**: `v18.0.0` or later ([Download Node.js](https://nodejs.org/))
2. **Rust & Cargo**: Latest stable Rust toolchain ([Install Rust](https://rustup.rs/))
3. **C++ Build Tools**: Visual Studio C++ Build Tools on Windows (required for Tauri)

---

## 🚀 App Installation & Setup

### 1. Open the project folder
```bash
cd "d:/As infotech/printer"
```

### 2. Install NPM dependencies
```bash
npm install
```

### 3. Start the Virtual Printer Application
```bash
npm run dev
```
*(Or `npm run tauri dev`)*

The desktop window will open. Click **▶ Start Server** to begin listening on `127.0.0.1:9100`.

---

## 🖨️ Installing as a Windows Printer

Installing the emulator as a system printer allows any Windows application (Word, Excel, Chrome, POS Software) to send print jobs directly to the virtual printer.

### Method 1: One-Click PowerShell Setup (Recommended)

Open **PowerShell as Administrator** and run:

```powershell
# 1. Create a Standard TCP/IP Printer Port pointing to localhost:9100
Add-PrinterPort -Name "VirtualPrinterPort" -PrinterHostAddress "127.0.0.1" -PortNumber 9100

# 2. Add the Virtual Printer using Generic / Text Only driver
Add-Printer -Name "Thermal Receipt Printer (XP-80)" -DriverName "Generic / Text Only" -PortName "VirtualPrinterPort"
```

> 💡 **Done!** The printer `"Thermal Receipt Printer (XP-80)"` will now appear in your Windows Printers list.

---

### Method 2: Windows GUI Setup

1. Open **Windows Settings** -> **Bluetooth & devices** -> **Printers & scanners**.
2. Click **Add device**, wait a few seconds, then click **Add manually**.
3. Select **Add a printer using an IP address or hostname** and click **Next**.
4. Configure the port:
   - **Device type**: `TCP/IP Device`
   - **Hostname or IP address**: `127.0.0.1`
   - **Port name**: `VirtualPrinter9100`
   - Uncheck *"Query the printer and automatically select the driver to use"*, then click **Next**.
5. Select Driver:
   - Manufacturer: **Generic**
   - Printers: **Generic / Text Only**
6. Name the printer: `Thermal Receipt Printer (XP-80)` or `POS-80 Series Thermal Printer`.
7. Click **Finish**.

---

## 🧪 Testing Your Printer

### 1. Built-in App Test Tools
In the left panel under **Test Tools**:
- Click **🧾 Send Test Receipt (QR + Barcode)**: Tests standard ESC/POS formatting, `GS ( k` QR codes, and Code128 barcodes.
- Click **📱 Send UPI QR Receipt**: Tests UPI payment receipt with `[UPI_QR:...]` tag rendering.

---

### 2. Windows Test Print Page
1. Ensure the Virtual Printer server is running in the app (**Running** status on port `9100`).
2. Open Windows **Settings** -> **Printers & scanners** -> **Thermal Receipt Printer (XP-80)**.
3. Click **Print test page**.
4. The test page will immediately appear in the **Receipt Preview** window.

---

### 3. Testing with PowerShell (Quick Script)

You can send a test thermal receipt directly from PowerShell without installing any drivers:

```powershell
$client = New-Object System.Net.Sockets.TcpClient("127.0.0.1", 9100)
$stream = $client.GetStream()

$esc = [char]0x1B
$gs  = [char]0x1D

$receipt = @"
$esc@$esca`x01$escE`x01PHARMA POS
$escE`x00123 Main Street, Medical City
Tel: +91-9999999999
--------------------------------
$esca`x00Item                 Qty   Price
--------------------------------
Paracetamol 500mg       2   50.00
Amoxicillin 500mg       1   80.00
--------------------------------
Total:                     130.00
--------------------------------
[UPI_QR:upi://pay?pa=store@upi&pn=PHARMA%20POS&am=130.00&cu=INR]
$esca`x01Thank you for your visit!
$esc d`x03$gs V`x00
"@

$bytes = [System.Text.Encoding]::UTF8.GetBytes($receipt)
$stream.Write($bytes, 0, $bytes.Length)
$stream.Close()
$client.Close()
Write-Host "Receipt sent to Virtual Printer!" -ForegroundColor Green
```

---

### 4. Testing with Node.js / JavaScript

```javascript
import net from 'net';

const client = new net.Socket();
client.connect(9100, '127.0.0.1', () => {
  const receipt = Buffer.concat([
    Buffer.from([0x1B, 0x40]),                        // ESC @ (Init)
    Buffer.from([0x1B, 0x61, 0x01, 0x1B, 0x45, 0x01]),// Center + Bold
    Buffer.from("AQNOOR PHARMACY\n"),
    Buffer.from([0x1B, 0x61, 0x00, 0x1B, 0x45, 0x00]),// Left + Normal
    Buffer.from("Paracetamol 650mg    x2    60.00\n"),
    Buffer.from("Total:                    60.00\n"),
    Buffer.from("[UPI_QR:upi://pay?pa=aqnoor@upi&pn=AQNOOR&am=60.00]\n"),
    Buffer.from([0x1B, 0x64, 0x03]),                  // Feed 3 lines
    Buffer.from([0x1D, 0x56, 0x00]),                  // Cut
  ]);

  client.write(receipt);
  client.end();
});
```

---

### 5. Testing with Python

```python
import socket

s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
s.connect(('127.0.0.1', 9100))

receipt = (
    b"\x1b@"                                           # Init
    b"\x1ba\x01\x1bE\x01PHARMA POS\n"                  # Center Bold
    b"\x1ba\x00\x1bE\x00"                              # Left Normal
    b"Invoice: INV-0042\n"
    b"Total: Rs.250.00\n"
    b"[UPI_QR:upi://pay?pa=pharmapos@upi&am=250.00]\n" # UPI QR
    b"\x1bd\x03\x1dVA\x00"                             # Feed & Cut
)

s.sendall(receipt)
s.close()
print("Receipt printed successfully!")
```

---

### 6. Testing with POS Apps (e.g. PharmaPOS)

1. Open your POS application (e.g. `D:\As infotech\phrama\my-app`).
2. Go to **Settings** -> **Printer Settings**.
3. Set:
   - **Printer**: `Thermal Receipt Printer (XP-80)` (or `127.0.0.1:9100` if TCP is supported)
   - **Paper Size**: `80mm Thermal` (or `58mm Thermal`)
   - **Print Datatype**: `RAW` (or `TEXT`)
   - **Print UPI QR**: `Enabled`
4. Make a sale or click **Test Print** in the POS app.
5. The complete receipt with the QR code and barcode will render immediately in the Virtual Printer window.

---

## 📱 Supported QR & Barcode Formats

| Format | ESC/POS Command | Description |
|---|---|---|
| **Epson QR** | `GS ( k pL pH cn fn ...` | Standard 5-field ESC/POS QR code command |
| **Raster QR** | `GS v 0 0 xL xH yL yH ...` | Monochrome bitmap raster QR image (used by PharmaPOS & RAW thermal drivers) |
| **2D Barcode** | `ESC Z v r k xL xH ...` | Chinese / Xprinter / Zjiang / POS-58 QR format |
| **Barcode QR** | `GS k 104 len ...` | 1D/2D Barcode mode QR Code |
| **Embedded Tag** | `[UPI_QR:uri]` / `[QR:uri]` | Auto-detected inline tag converted to visual QR code |
| **1D Barcodes** | `GS k n data...` | CODE128, CODE39, EAN13, EAN8, UPCA, UPCE, ITF, Codabar |

---

## 🛠 Project Architecture

```
printer/
├── package.json              # App metadata and scripts
├── web/                      # Frontend UI (HTML, CSS, JS)
│   ├── index.html            # Main UI layout & responsive control dashboard
│   └── src/
│       ├── main.js           # IPC communication, event handlers & tab controls
│       └── style.css         # Dark theme UI styling & live thermal paper preview
└── src-tauri/                # Rust Backend & ESC/POS Core
    ├── Cargo.toml            # Rust dependencies (rxing, qrcode, tokio, image, tauri)
    ├── tauri.conf.json       # Tauri window & security configuration
    └── src/
        ├── tcp_server.rs     # Async non-blocking TCP socket server (port 9100)
        ├── escpos_parser.rs  # Byte-level ESC/POS state machine parser (QR, barcodes, raster)
        ├── qrcode.rs         # QR code generator & PNG base64 renderer
        ├── barcode.rs        # 1D/2D Barcode generator
        ├── renderer.rs       # HTML & thermal CSS receipt renderer
        ├── test_receipt.rs   # Pre-built test receipt generators
        └── tests/            # Automated Rust unit test suite (19 tests)
```

---

## 🧪 Automated Unit Tests

The project includes an automated Rust test suite covering ESC/POS byte parsing, all QR formats, barcodes, paper cuts, and styling.

To run the unit tests:

```bash
cd "d:/As infotech/printer/src-tauri"
cargo test
```

---

## 📦 Production Build & Releases

### Local Windows Build
To compile a self-contained Windows desktop installer/executable locally:

```bash
cd "d:/As infotech/printer"
npm run build
```

The compiled release will be located at:
`src-tauri/target/release/aqnoor-virtual-printer.exe`

### Automated GitHub Actions Release (Windows)
The repository includes a dedicated Windows release workflow in [`.github/workflows/release.yml`](file:///.github/workflows/release.yml).

To publish a new GitHub Release with installer assets (`.msi` and `.exe`):

1. **Tag and Push:**
   ```bash
   git tag v1.0.0
   git push origin v1.0.0
   ```
2. **Or Trigger Manually:**
   - Go to your GitHub repository -> **Actions** -> **Release Windows App** -> **Run workflow**.
   - GitHub Actions will automatically compile the Rust backend, run all 19 unit tests, and attach the Windows installer `.exe` / `.msi` to the GitHub Release.

---

## ❓ Troubleshooting & FAQ

#### Q: The printer says "Stopped" and doesn't receive jobs.
- Click **▶ Start Server** in the left panel.
- Ensure no other software is using port `9100` (e.g. `netstat -ano | findstr 9100`).

#### Q: Receipts show `[UPI_QR:...]` or raw characters.
- Ensure you are running the latest build. The parser now automatically converts `[UPI_QR:...]`, `GS ( k`, `GS v 0`, and `ESC Z` into clean visual QR codes.

#### Q: How do I switch between 80mm and 58mm receipts?
- Click the **80 mm** or **58 mm** toggle button under **Server Configuration**. The preview dynamically adjusts width and font scaling.

---

## 📄 License

MIT License © AqNoor / AS Infotech
