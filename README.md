# AnyViewer

A modern, secure, and high-performance remote desktop application built with Rust and Tauri.

## Features

- **Cross-platform**: Works on Windows, macOS, and Linux
- **High Performance**: Built with Rust for maximum performance and memory safety
- **Modern UI**: React-based frontend with Tailwind CSS
- **Secure**: End-to-end encryption with RSA and AES-256
- **Real-time**: Low-latency screen sharing with adaptive quality
- **Lightweight**: Small binary size compared to Electron alternatives

## Architecture

### Backend (Rust)
- **Screen Capture**: Cross-platform screen capture with hardware acceleration
- **Video Codec**: Supports JPEG, PNG, and H.264 encoding
- **Network Layer**: WebSocket-based communication with relay server support
- **Input Handling**: Mouse and keyboard input forwarding
- **Security**: RSA key exchange and AES-256 session encryption

### Frontend (React + TypeScript)
- **Modern UI**: Clean, responsive interface built with React
- **Real-time Updates**: Live connection status and performance metrics
- **Settings Management**: Comprehensive configuration options
- **Theme Support**: Light, dark, and system themes

## Getting Started

### Prerequisites

- Rust (latest stable)
- Node.js (16+)
- npm or yarn

### Installation

1. Clone the repository:
```bash
git clone <repository-url>
cd AnyViewer
```

2. Install frontend dependencies:
```bash
npm install
```

3. Install Rust dependencies:
```bash
cargo fetch
```

### Development

1. Start the development server:
```bash
npm run tauri dev
```

This will start both the Vite dev server for the frontend and compile the Rust backend.

### Building

1. Build for production:
```bash
npm run tauri build
```

The built application will be available in `src-tauri/target/release/bundle/`.

## Project Structure

```
AnyViewer/
├── src/                    # Frontend source (React/TypeScript)
│   ├── components/         # React components
│   ├── pages/             # Application pages
│   ├── hooks/             # Custom React hooks
│   ├── utils/             # Utility functions
│   └── types/             # TypeScript type definitions
├── src-tauri/             # Backend source (Rust)
│   ├── src/
│   │   ├── capture/       # Screen capture module
│   │   ├── network/       # Network communication
│   │   ├── codec/         # Video encoding/decoding
│   │   ├── input/         # Input handling
│   │   ├── security/      # Encryption and authentication
│   │   ├── config/        # Configuration management
│   │   └── utils/         # Utility modules
│   ├── Cargo.toml         # Rust dependencies
│   └── tauri.conf.json    # Tauri configuration
└── package.json           # Frontend dependencies
```

## Key Technologies

- **Backend**: Rust, Tauri, Tokio, WebSockets
- **Frontend**: React, TypeScript, Tailwind CSS, Vite
- **Security**: RSA, AES-256-GCM, TLS
- **Video**: JPEG, PNG, H.264 (planned)
- **Network**: WebSocket, UDP (planned)

## Usage

### Starting a Host Session

1. Open AnyViewer
2. Go to "Host Session"
3. Click "Start Hosting"
4. Share the generated Session ID with remote users

### Connecting to a Remote Desktop

1. Open AnyViewer
2. Go to "Connect to Remote Desktop"
3. Enter the Session ID provided by the host
4. Click "Connect"

## Configuration

Settings can be configured through the Settings page or by editing the configuration file:

- **macOS**: `~/Library/Application Support/com.anyviewer.app/config.toml`
- **Windows**: `%APPDATA%\com.anyviewer.app\config.toml`
- **Linux**: `~/.config/anyviewer/config.toml`

## Security

AnyViewer implements multiple layers of security:

1. **RSA Key Exchange**: 2048-bit RSA keys for secure session establishment
2. **AES-256 Encryption**: All session data encrypted with AES-256-GCM
3. **Session Management**: Automatic session timeout and cleanup
4. **Rate Limiting**: Protection against brute force attacks

## Performance

- **Low Latency**: Optimized for real-time screen sharing
- **Adaptive Quality**: Automatic quality adjustment based on network conditions
- **Hardware Acceleration**: GPU encoding support (where available)
- **Memory Efficient**: Rust's zero-cost abstractions and memory safety

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## Development Commands

```bash
# Start development server
npm run tauri dev

# Build for production
npm run tauri build

# Run frontend linting
npm run lint

# Run type checking
npm run type-check

# Install Tauri CLI
cargo install tauri-cli
```

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Roadmap

- [ ] File transfer support
- [ ] Multi-monitor support
- [ ] Mobile app support
- [ ] Hardware acceleration
- [ ] Cloud relay servers
- [ ] Audio forwarding
- [ ] Recording functionality
- [ ] Plugin system

## Support

For issues and support, please create an issue on the GitHub repository.