# Third-party notices

Traceglass uses third-party libraries and redistributes selected fonts and icons.
Their licenses apply to their respective components. This file does not assign
a license to Traceglass's original code.

| Component | Use | License / notice |
| --- | --- | --- |
| GPUI Kit 0.7.3 and GPUI | UI framework and browser rendering | Apache-2.0; [license text](GPUI-LICENSE-APACHE) |
| IBM Plex Sans | Embedded interface font | SIL Open Font License 1.1; [font notices and license](fonts/OFL.txt) |
| JetBrains Mono | Embedded monospace font | SIL Open Font License 1.1; [font notices and license](fonts/OFL.txt) |
| Lucide | Selected embedded icons from `gpui-kit-assets` | ISC; MIT for Feather-derived icons; [copyright notices and licenses](LICENSE-LUCIDE) |

The font files were obtained from the GPUI Kit WebAssembly example assets.
The supplied IBM Plex Sans binary is version 3.005 and includes
`Copyright 2018 IBM Corp. All rights reserved.`; JetBrains Mono is version 2.304.
These embedded notices are preserved alongside the upstream font license.
The supplied font notice also retains the upstream statements for Source, Inter,
and Noto font families used by the GPUI font fallback assets.

The Lucide license is copied unchanged from `gpui-kit-assets` 0.7.3.
Build output includes the GPUI, font, and icon notices alongside the application.

Other dependencies are recorded in `Cargo.lock` and `package-lock.json`;
their package distributions contain their respective license texts.

Upstream projects:

- [GPUI Kit](https://github.com/longbridge/gpui-kit)
- [IBM Plex](https://github.com/IBM/plex)
- [JetBrains Mono](https://github.com/JetBrains/JetBrainsMono)
- [Lucide](https://lucide.dev/license)
