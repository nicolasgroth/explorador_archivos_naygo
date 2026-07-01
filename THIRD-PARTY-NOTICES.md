# Avisos de software de terceros

<!-- Naygo — avisos de licencias de dependencias de terceros.
     Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License (el proyecto).
     Archivo generado con `cargo license`; ver instrucciones más abajo para regenerarlo. -->

Naygo se distribuye bajo licencia **MIT** (ver `LICENSE`). El binario incluye o enlaza
software de terceros, cada uno bajo su propia licencia. Esta lista cubre las dependencias
que entran en la compilación para Windows (x86_64-pc-windows-msvc): **382 paquetes**.

Todas son licencias permisivas (MIT, Apache-2.0, BSD, ISC, Zlib, etc.) o, en el caso de
**Slint**, su licencia *royalty-free* (ver más abajo). Ninguna obliga a Naygo a cambiar su
licencia ni impone regalías.

Esta lista se genera con `cargo license`. Para regenerarla:

```
cargo license --avoid-dev-deps --avoid-build-deps --filter-platform x86_64-pc-windows-msvc
```

---

## Slint (UI)

La interfaz usa [Slint](https://slint.dev). Slint se ofrece bajo triple licencia
(`GPL-3.0 OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0`); Naygo lo
usa bajo la **Slint Royalty-free License 2.0**, que permite uso gratuito y sin regalías en
aplicaciones de escritorio a cambio de una atribución visible. Naygo la incluye en la
sección «Acerca de» («Hecho con Rust + Slint»). Más información: <https://slint.dev>.

---

## Resaltado de sintaxis (syntect)

La vista previa de código resalta la sintaxis con [syntect](https://github.com/trishume/syntect)
(licencia MIT) y sus gramáticas/temas embebidos. Todas sus dependencias son permisivas.

---

## Vista previa de archivos comprimidos (tar, flate2, miniz_oxide)

La vista previa de `.tar`, `.tar.gz` y `.zip` usa:

- **tar** (<https://crates.io/crates/tar>) — licencia MIT/Apache-2.0. Lee archivos TAR de forma secuencial, en puro Rust, sin dependencias nativas.
- **flate2** (<https://crates.io/crates/flate2>) — licencia MIT/Apache-2.0. Descompresión gzip (`.tar.gz`). Backend: **miniz_oxide** (MIT/Zlib/Apache-2.0), implementación pura en Rust de miniz.

Todas son licencias permisivas. Ninguna impone regalías ni restricciones de distribución.

---

## Íconos (sets de fábrica)

Los sets de íconos de fábrica se generan a partir de estas librerías de código abierto,
embebidas como PNG en el binario:

- **Lucide** (<https://lucide.dev>) — licencia ISC. Sets `lucide` y `mono`.
- **Tabler Icons** (<https://tabler.io/icons>) — licencia MIT. Set `tabler`.
- **Material Symbols / Material Design Icons** (<https://fonts.google.com/icons>) — licencia Apache-2.0. Set `material`.
- **Flat Color Icons** (<https://github.com/icons8/flat-color-icons>) — licencia MIT. Set `flat-color`.

Todas son licencias permisivas. Los íconos importados por el usuario (sets `.naygoset`
o packs sueltos) son responsabilidad de quien los distribuye.

---

## Dependencias por licencia

### (Apache-2.0 OR MIT) AND BSD-3-Clause  (1)

encoding_rs 0.8.35

### (Apache-2.0 OR MIT) AND Unicode-3.0  (1)

unicode-ident 1.0.24

### 0BSD OR Apache-2.0 OR MIT  (1)

adler2 2.0.1

### Apache-2.0  (7)

glutin 0.32.3, glutin_egl_sys 0.7.1, glutin_wgl_sys 0.6.1, linked_hash_set 0.1.6, unicode-linebreak 0.1.5, winit 0.30.13, zopfli 0.8.3

### Apache-2.0 AND MIT  (1)

dpi 0.1.2

### Apache-2.0 OR BSD-2-Clause OR MIT  (2)

zerocopy 0.8.50, zerocopy-derive 0.8.50

### Apache-2.0 OR BSD-3-Clause  (2)

moxcms 0.8.1, pxfm 0.1.29

### Apache-2.0 OR BSD-3-Clause OR MIT  (2)

num_enum 0.7.6, num_enum_derive 0.7.6

### Apache-2.0 OR CC0-1.0  (1)

imgref 1.12.2

### Apache-2.0 OR MIT  (226)

aes 0.8.4, aligned 0.4.3, allocator-api2 0.2.21, annotate-snippets 0.12.16, anstyle 1.0.14, anyhow 1.0.102, arrayvec 0.7.6, as-slice 0.2.1, auto_enums 0.8.8, base64 0.22.1, bit-set 0.8.0, bit-vec 0.8.0, bit_field 0.10.3, bitflags 2.12.1, bitstream-io 4.10.0, block-buffer 0.10.4, block-padding 0.3.3, borsh 1.6.1, bumpalo 3.20.3, by_address 1.2.1, bytecount 0.6.9, cbc 0.1.2, cff-parser 0.1.0, cfg-if 1.0.4, chacha20 0.10.0, chrono 0.4.45, cipher 0.4.4, const-field-offset 0.2.0, const-field-offset-macro 0.2.0, copypasta 0.10.2, countme 3.0.1, cpufeatures 0.2.17, cpufeatures 0.3.0, crc32fast 1.5.0, critical-section 1.2.0, crossbeam-channel 0.5.15, crossbeam-deque 0.8.6, crossbeam-epoch 0.9.18, crossbeam-utils 0.8.21, crypto-common 0.1.7, data-url 0.3.2, deranged 0.5.8, derive_utils 0.15.1, digest 0.10.7, displaydoc 0.2.6, either 1.16.0, equivalent 1.0.2, euclid 0.20.14, euclid 0.22.14, fdeflate 0.3.7, field-offset 0.3.6, file-id 0.2.3, filetime 0.2.29, flate2 1.1.9, fnv 1.0.7, font-types 0.11.3, fontique 0.8.0, form_urlencoded 1.2.2, getopts 0.2.24, getrandom 0.3.4, getrandom 0.4.2, gif 0.14.2, global-hotkey 0.6.4, half 2.7.1, hashbrown 0.14.5, hashbrown 0.16.1, hashbrown 0.17.1, heck 0.5.0, htmlparser 0.2.1, idna 1.1.0, idna_adapter 1.2.2, image 0.25.10, image-webp 0.2.4, indexmap 2.14.0, inout 0.1.4, integer-sqrt 0.1.5, itertools 0.14.0, itoa 1.0.18, keyboard-types 0.7.0, kurbo 0.13.1, lazy_static 1.5.0, libc 0.2.186, linebender_resource_handle 0.1.1, linked-hash-map 0.5.6, log 0.4.32, lyon_algorithms 1.0.20, lyon_extra 1.1.0, lyon_geom 1.0.19, lyon_path 1.0.19, md-5 0.10.6, memmap2 0.9.10, muda 0.18.0, muda 0.19.2, no_std_io2 0.9.4, notify-debouncer-full 0.5.0, notify-types 2.1.0, num-bigint 0.4.6, num-conv 0.2.2, num-derive 0.4.2, num-integer 0.1.46, num-rational 0.4.2, num-traits 0.2.19, once_cell 1.21.4, parlance 0.1.0, parley 0.8.0, parley_data 0.8.0, paste 1.0.15, pastey 0.1.1, percent-encoding 2.3.2, pin-project 1.1.13, pin-project-internal 1.1.13, pin-project-lite 0.2.17, pin-utils 0.1.0, png 0.18.1, polycool 0.4.0, portable-atomic 1.13.1, postscript 0.14.1, powerfmt 0.2.0, ppv-lite86 0.2.21, proc-macro-crate 3.5.0, proc-macro2 1.0.106, profiling 1.0.18, profiling-procmacros 1.0.18, qoi 0.4.1, quick-error 2.0.1, quote 1.0.45, rand 0.9.4, rand 0.10.1, rand_chacha 0.9.0, rand_core 0.9.5, rand_core 0.10.1, rangemap 1.7.1, rayon 1.12.0, rayon-core 1.13.0, read-fonts 0.37.0, regex 1.12.3, regex-automata 0.4.14, regex-syntax 0.8.10, resvg 0.47.0, rowan 0.16.1, roxmltree 0.21.1, rustc-hash 1.1.0, rustversion 1.0.22, scoped-tls-hkt 0.1.5, scopeguard 1.2.0, serde 1.0.228, serde_core 1.0.228, serde_derive 1.0.228, serde_json 1.0.150, sha2 0.10.9, simplecss 0.2.2, siphasher 1.0.3, skrifa 0.40.0, smallvec 1.15.1, smol_str 0.2.2, smol_str 0.3.6, snafu 0.8.9, snafu-derive 0.8.9, softbuffer 0.4.8, spin_on 0.1.1, stable_deref_trait 1.2.1, stringprep 0.1.5, svgtypes 0.16.1, swash 0.2.9, syn 2.0.117, sys-locale 0.3.2, tar 0.4.46, text-size 1.1.1, thiserror 2.0.18, thiserror-impl 2.0.18, time 0.3.49, time-core 0.1.9, time-macros 0.2.29, toml_datetime 1.1.1+spec-1.1.0, toml_edit 0.25.12+spec-1.1.0, toml_parser 1.1.2+spec-1.1.0, toml_writer 1.1.1+spec-1.1.0, tray-icon 0.24.1, ttf-parser 0.25.1, typed-index-collections 3.5.0, typenum 1.20.1, unicase 2.9.0, unicode-bidi 0.3.18, unicode-bidi-mirroring 0.4.0, unicode-ccc 0.4.0, unicode-normalization 0.1.25, unicode-properties 0.1.4, unicode-script 0.5.8, unicode-segmentation 1.13.3, unicode-vo 0.1.0, unicode-width 0.2.2, unicode-xid 0.2.6, unty 0.0.4, url 2.5.8, usvg 0.47.0, utf8_iter 1.0.4, vtable 0.4.0, vtable-macro 0.4.0, wasm-bindgen 0.2.122, wasm-bindgen-macro 0.2.122, wasm-bindgen-macro-support 0.2.122, wasm-bindgen-shared 0.2.122, webbrowser 1.2.1, weezl 0.1.12, windows 0.62.2, windows-collections 0.3.2, windows-core 0.62.2, windows-future 0.3.2, windows-implement 0.60.2, windows-interface 0.59.3, windows-link 0.2.1, windows-numerics 0.3.1, windows-result 0.4.1, windows-strings 0.5.1, windows-sys 0.52.0, windows-sys 0.59.0, windows-sys 0.60.2, windows-sys 0.61.2, windows-targets 0.52.6, windows-targets 0.53.5, windows-threading 0.2.1, windows_x86_64_msvc 0.52.6, windows_x86_64_msvc 0.53.1, yaml-rust 0.4.5, yazi 0.2.1, zeno 0.3.3

### Apache-2.0 OR MIT OR Zlib  (11)

bytemuck 1.25.0, bytemuck_derive 1.10.2, cursor-icon 1.2.0, glow 0.17.0, miniz_oxide 0.8.9, raw-window-handle 0.6.2, tinyvec 1.11.0, tinyvec_macros 0.1.1, zune-core 0.5.1, zune-inflate 0.2.54, zune-jpeg 0.5.15

### BSD-2-Clause  (4)

arrayref 0.3.9, av1-grain 0.2.5, rav1e 0.8.1, v_frame 0.3.9

### BSD-3-Clause  (6)

avif-serialize 0.8.9, exr 1.74.0, lebe 0.5.3, ravif 0.13.0, tiny-skia 0.12.0, tiny-skia-path 0.12.0

### BSL-1.0  (2)

clipboard-win 5.4.1, error-code 3.3.2

### CC0-1.0  (1)

notify 8.2.0

### GPL-3.0 OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0  (10)

i-slint-backend-selector 1.16.1, i-slint-backend-winit 1.16.1, i-slint-common 1.16.1, i-slint-compiler 1.16.1, i-slint-core 1.16.1, i-slint-core-macros 1.16.1, i-slint-renderer-skia 1.16.1, i-slint-renderer-software 1.16.1, slint 1.16.1, slint-macros 1.16.1

### ISC  (1)

libloading 0.8.9

### MIT  (69)

adobe-cmap-parser 0.4.1, aligned-vec 0.6.4, arg_enum_proc_macro 0.3.4, av-scenechange 0.14.1, bincode 1.3.3, bincode 2.0.1, bytes 1.11.1, clru 0.6.3, color_quant 1.1.0, convert_case 0.10.0, core_maths 0.1.1, derive_more 2.1.1, derive_more-impl 2.1.1, ecb 0.1.2, equator 0.4.2, equator-macro 0.4.2, fancy-regex 0.16.2, fax 0.2.7, float-cmp 0.9.0, fontdb 0.23.0, generic-array 0.14.7, glutin-winit 0.5.0, grid 1.0.1, harfrust 0.5.2, imagesize 0.14.0, libm 0.2.16, loop9 0.1.5, lopdf 0.38.0, lopdf 0.41.0, maybe-rayon 0.1.1, memoffset 0.9.1, natord 1.0.9, new_debug_unreachable 1.0.6, nom 8.0.0, nom_locate 5.0.0, noop_proc_macro 0.3.0, pdf-extract 0.10.0, pico-args 0.5.0, pin-weak 1.1.0, plist 1.9.0, pom 1.1.0, pulldown-cmark 0.13.4, pulldown-cmark-escape 0.11.0, quick-xml 0.39.4, rfd 0.15.4, rgb 0.8.53, rspolib 0.1.2, rustybuzz 0.20.1, simd-adler32 0.3.9, simd_helpers 0.1.0, skia-bindings 0.90.0, skia-safe 0.90.0, slab 0.4.12, strict-num 0.1.1, strum 0.28.0, strum_macros 0.28.0, synstructure 0.13.2, syntect 5.3.0, taffy 0.9.2, tiff 0.11.3, tracing 0.1.44, tracing-attributes 0.1.31, tracing-core 0.1.36, type1-encoding-parser 0.1.1, winnow 1.0.3, xmlwriter 0.1.0, y4m 0.8.0, zip 2.4.2, zmij 1.0.21

### MIT OR Unlicense  (10)

aho-corasick 1.1.4, byteorder-lite 0.1.0, jiff 0.2.28, jiff-static 0.2.28, jiff-tzdb 0.1.6, jiff-tzdb-platform 0.1.3, memchr 2.8.1, same-file 1.0.6, walkdir 2.5.0, winapi-util 0.1.11

### Unicode-3.0  (22)

icu_collections 2.2.0, icu_locale 2.2.0, icu_locale_core 2.2.0, icu_locale_data 2.2.0, icu_normalizer 2.2.0, icu_normalizer_data 2.2.0, icu_properties 2.2.0, icu_properties_data 2.2.0, icu_provider 2.2.0, icu_segmenter 2.2.0, icu_segmenter_data 2.2.0, litemap 0.8.2, potential_utf 0.1.5, tinystr 0.8.3, writeable 0.6.3, yoke 0.8.3, yoke-derive 0.8.2, zerofrom 0.1.8, zerofrom-derive 0.1.7, zerotrie 0.2.4, zerovec 0.11.6, zerovec-derive 0.11.3

### Zlib  (2)

foldhash 0.2.0, slotmap 1.1.1
