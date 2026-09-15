# Approved icon family

Approved 2026-09-06. `AppIcon.icon` is the red shipping master;
`AppIcon-cobalt.icon` is the approved alternate. Both include native light/dark
specializations. PNG previews are references, not layered source documents.

Run `DEVELOPER_DIR=/Applications/Xcode-beta.app/Contents/Developer bash docs/icon/build-icons.sh`
after editing the shipping master (requires npm dependencies and full Xcode).
Tauri packages the compiled `src-tauri/Assets.car` via `bundle.resources` and
selects `AppIcon` via `src-tauri/Info.plist`. PNG/ICO/ICNS remain static fallbacks.
Mobile assets are deliberately unchanged: macOS-masked exports are not mobile masters.

Native builds compile `native/AppIcon.icon`; keep that copy synchronized with
the shipping master here. The runtime must not override the packaged icon with
an NSImage, which would flatten its system-controlled appearance.

The box retains the approved size and barley placement, with no extra face
outlines and quiet specular treatment. No small-size redesign was introduced.
