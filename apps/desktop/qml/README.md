# Desktop QML boundary

Future QML files own desktop presentation only. They communicate through the thin Qt/Rust bridge and never directly control agents, system services, or platform adapters. QML remains isolated within `apps/desktop/`; the desktop UI is not built in this initialization iteration.
