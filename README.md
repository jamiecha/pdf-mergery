# PDF Mergery

A Tauri application for merging PDF files, built with vanilla HTML, CSS, and TypeScript.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## Development

### Prerequisites
- [Node.js](https://nodejs.org/) (v16 or later)
- [Rust](https://rustup.rs/)

### Running the App
```bash
# Install dependencies
npm install

# Start the development server
npm run dev

# Run the Tauri app in development mode
npm run tauri dev
```

## Building

```bash
# Build the frontend
npm run build

# Build the Tauri app for production
npm run tauri build
```

This will generate installers in `src-tauri/target/release/bundle/`.

## Testing

```bash
# Run Rust tests
cd src-tauri
cargo test

# Run frontend tests (if any)
npm test
```

## Releasing

1. Ensure all tests pass.
2. Build the app: `npm run tauri build`
3. The installers will be available in `src-tauri/target/release/bundle/`.
4. Upload the installers to your preferred distribution platform (e.g., GitHub Releases).

## License

[Add your license here]
