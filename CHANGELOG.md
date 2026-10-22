# Changelog

## [0.1.0](https://github.com/KarelZe/dame-rs/compare/0.1.0...0.1.0) (2026-10-22)


### Features

* add a score icon and shorten the expected line in the macOS status bar ✨ ([4cebc47](https://github.com/KarelZe/dame-rs/commit/4cebc4796e22e321854a5e0102371aa7e38af40b))
* add alpha-beta search, evaluation and game rules ✨ ([8980c0e](https://github.com/KarelZe/dame-rs/commit/8980c0eed4ac663ebde706142ec196e28a415cfe))
* add bitboard move generator with perft validation ✨ ([63813d8](https://github.com/KarelZe/dame-rs/commit/63813d8e082e8d9eb7590cfca48bbaeec8aec53a))
* add dame CLI with Hub protocol and terminal play ✨ ([4d1f851](https://github.com/KarelZe/dame-rs/commit/4d1f85196df069b687e06c8a64665488051d3af3))
* add native macOS app driving Hub engines ✨ ([87a6db4](https://github.com/KarelZe/dame-rs/commit/87a6db424381f4e5924fbc3a393f35080ae30a30))
* add pythonic Python bindings for board, engine and book ✨ ([e1af4a4](https://github.com/KarelZe/dame-rs/commit/e1af4a413a0f444019f4c36679df55eda4771fd9))
* add Reload Engine shortcut and app icon to the macOS app ✨ ([a360f93](https://github.com/KarelZe/dame-rs/commit/a360f93400daeba6b9d7feefb1cc1ffb4c0b2a99))
* build opening books from PDN game collections ✨ ([ff21c97](https://github.com/KarelZe/dame-rs/commit/ff21c97340d5c926bb0332a59971f4935cba2ca4))
* build the macOS app as a DMG and attach it to GitHub releases ✨ ([e9319b2](https://github.com/KarelZe/dame-rs/commit/e9319b2261dd01d5b627daff094cae5e7d3f5322))
* open an opening book from the macOS File menu and set it via DAME_BOOK in the CLI ✨ ([10b3c80](https://github.com/KarelZe/dame-rs/commit/10b3c80e36f9c8270fe27db31e8e5ca958a5ccd2))
* polish the macOS status bar with tooltips and icons ✨ ([d07b472](https://github.com/KarelZe/dame-rs/commit/d07b47226964adf6836e81d0dc0a77415620f99a))
* set the opening book depth via DAME_BOOK_PLIES and the macOS status bar ✨ ([f8b2ee3](https://github.com/KarelZe/dame-rs/commit/f8b2ee36629645734358c6c6b738bcab8d179f98))
* show the version and project link in the macOS About panel ✨ ([90d7e52](https://github.com/KarelZe/dame-rs/commit/90d7e524853b861367db3f5aca27c2fb31949f32))


### Bug Fixes

* restore the think example deleted by accident 🐛 ([29a3c5a](https://github.com/KarelZe/dame-rs/commit/29a3c5ae3690985492f8ec6cff8ff8e27450c314))


### Performance

* replay book moves on a position without a second legality check ⚡ ([12a517f](https://github.com/KarelZe/dame-rs/commit/12a517f65da98aaef854a2b7f166babc8d09ba87))
