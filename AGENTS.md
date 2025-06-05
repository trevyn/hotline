This repository depends on SDL2 for the runtime. Building requires the SDL2 development headers to be installed. On Debian-based systems run:

```bash
sudo apt-get update && sudo apt-get install -y libsdl2-dev
```

Make sure SDL2 is installed before running `cargo build` or the linker will fail to resolve SDL symbols.
