// SPDX-License-Identifier: GPL-3.0-or-later
//! The C library trigonometry Blender's OpenSubdiv build calls in `CatmarkLimits<float>` and
//! `GregoryConverter<float>`: `cos`, `sin` and `cosf` of the platform libm, and Apple's
//! `__sincos_stret` where clang merges a `sin`/`cos` pair of one argument
//! (`ComputeBoundaryPointWeights`: `cos(theta)`/`sin(theta)` and the edge angles; verified in
//! the disassembly of Blender 5.2.4's `libosdCPU.dylib`).
//!
//! Deliberate exception to rule 2.2.12 (portable `libm::*`): the face-varying stencils must be
//! bit-identical to Blender's, which uses the platform libm, so this module calls the platform
//! functions (`f64::cos` / `f64::sin` / `f32::cos` lower to libSystem's `cos`/`sin`/`cosf`;
//! `black_box` keeps LLVM from merging a pair into `__sincos_stret` in some builds only).
//! `__sincos_stret` differs from the separate calls by one ulp for a few arguments; those are
//! tabulated below for every boundary-span angle `(pi / n) * k`, `n <= 256`, `k <= n + 2`
//! (generated with Blender's platform libm, macOS 26 arm64). `cosf` and the cosine of
//! `__sincosf_stret` agree on all of `GregoryConverter`'s angles (`pi / n`, `2 pi / n`).

/// `(argument, sin, cos)` bit patterns where `__sincos_stret` differs from `sin`/`cos`.
const SINCOS_EXCEPTIONS: &[(u64, u64, u64)] = &[
    (0x3fb7c1101bcbc73f, 0x3fb7b85726c680d8, 0x3fefdcc251e85326),
    (0x3fc70d3b4eeb5f2b, 0x3fc6ed6266798ab6, 0x3fef7b837b2cb7d0),
    (0x3fc7f4d1100cb211, 0x3fc7d11355ffc8fc, 0x3fef70f12b33a916),
    (0x3fcb231f144fe54e, 0x3fcaef31f1abb538, 0x3fef4894ee265407),
    (0x3fd0ebca29869d18, 0x3fd0b97f967b54f2, 0x3feee3578abd690a),
    (0x3fd18d931647fefd, 0x3fd155734246e1e2, 0x3feecdd15d5f94ed),
    (0x3fd263caa18f8b25, 0x3fd22346886535c4, 0x3feeb022a41d7f9b),
    (0x3fd2e96979b49175, 0x3fd2a3430848538a, 0x3fee9cf0a61696c5),
    (0x3fd440372bf4a193, 0x3fd3ea23a2ca87be, 0x3fee6950496c83fa),
    (0x3fd44f2d5b5de135, 0x3fd3f85b21221fda, 0x3fee66fb8b7249f1),
    (0x3fd6183f69067c0a, 0x3fd5a88f559b26f0, 0x3fee1ca71d6c9c5a),
    (0x3fd85804a8de2962, 0x3fd7c2d2e03c27ae, 0x3fedb67d3bcdbaa2),
    (0x3fd88ea37f487a3b, 0x3fd7f581a246b528, 0x3fedac4ebd9fed68),
    (0x3fd8e90b3a99a94c, 0x3fd8493e6cd36dbc, 0x3fed9b451eb8f28c),
    (0x3fd93c07bf5c4a4c, 0x3fd895f16f6487a0, 0x3fed8b6d6a20cf5c),
    (0x3fdd0ad2c7c63f7d, 0x3fdc0e45dabe05c8, 0x3fecc2ebbb5638ca),
    (0x3fdd4b35e4c2bf95, 0x3fdc48168ad2380a, 0x3fecb4c05344225c),
    (0x3fde28c731eb6950, 0x3fdd0e2e2b44de00, 0x3fec83201d3d2c6d),
    (0x3fde51a0f7f0b86f, 0x3fdd328e2f5ac8a8, 0x3fec79d4718f48ae),
    (0x3fde64a10c99e32f, 0x3fdd43754e3c30d8, 0x3fec757daa8cf3f9),
    (0x3fde84b12f775b54, 0x3fdd5ff578561768, 0x3fec6e258ad9b3a1),
    (0x3fdeb03af868cdf8, 0x3fdd869d15d2b146, 0x3fec64211c38cea6),
    (0x3fdf234e0940139f, 0x3fddec85e5f5f198, 0x3fec4967eed9d223),
    (0x3fdf3d9e66b49e0c, 0x3fde03c60c607546, 0x3fec433eb613acb4),
    (0x3fdf827c0df2219c, 0x3fde408749beb88c, 0x3fec330861a28bf9),
    (0x3fdf848f9756e160, 0x3fde425bad91a170, 0x3fec328abdabe640),
    (0x3fdfccf4dd8fbbab, 0x3fde821339882f30, 0x3fec215b727ad6de),
    (0x3fdfd29fd75bd954, 0x3fde870e9dfcad70, 0x3fec200181326202),
    (0x3fdfe2efec9a296d, 0x3fde95640367fa04, 0x3fec1c1c9c0cde31),
    (0x3fe0282191998ab4, 0x3fdef5401024c4f4, 0x3fec01d48cb95263),
    (0x3fe04d64c109a0b6, 0x3fdf3664da86a9de, 0x3febefbb4b1f28ee),
    (0x3fe0604717a16afe, 0x3fdf57584d6234c4, 0x3febe680bfad984f),
    (0x3fe067961f6ffb28, 0x3fdf6416240d4852, 0x3febe2ebc41dc163),
    (0x3fe0789eed767171, 0x3fdf81c2231a15bc, 0x3febda8ce9971ab7),
    (0x3fe0ab18c1d5abd6, 0x3fdfd979db5468f4, 0x3febc190f39c9c6a),
    (0x3fe136d639b7768f, 0x3fe06558bb4fe6be, 0x3feb7afe8d78e60d),
    (0x3fe16a312c476ddc, 0x3fe0915dab145a44, 0x3feb608b16926b0e),
    (0x3fe1cc1efd33f3d6, 0x3fe0e4d7af0e26b8, 0x3feb2d576ec69869),
    (0x3fe27bb7af9f3262, 0x3fe178f8108e1502, 0x3feacf0bff7d84de),
    (0x3fe28f3f812e6831, 0x3fe18951869c647e, 0x3feac45d05dab479),
    (0x3fe297623e55800b, 0x3fe1901f1b8f20dc, 0x3feabfe6ca7fe099),
    (0x3fe2bb83ac857f6d, 0x3fe1ae47b8e99f74, 0x3feaac0145b08b55),
    (0x3fe2eac82ee8fe28, 0x3fe1d59a1ccbe454, 0x3fea91c66297a3e3),
    (0x3fe2ef2011905913, 0x3fe1d935240b628a, 0x3fea8f5a75a24bf5),
    (0x3fe34f9fd89c8500, 0x3fe228fba09d272e, 0x3fea590f1effaa6d),
    (0x3fe3505735681891, 0x3fe229929929254e, 0x3fea58a70e46c610),
    (0x3fe35b5d6578e18a, 0x3fe232a516b00a56, 0x3fea5263b0b0c5a6),
    (0x3fe3669518a06aab, 0x3fe23bde1b801fca, 0x3fea4c00fd1f1075),
    (0x3fe3c1473a690307, 0x3fe2861ca0ddc6f8, 0x3fea17e9c2bece48),
    (0x3fe3e591a2b5f908, 0x3fe2a3a844564aa4, 0x3fea02d70cdf74db),
    (0x3fe3f181fe765bc0, 0x3fe2ad5b51d10428, 0x3fe9fbe0fae0296a),
    (0x3fe400ec9be2abba, 0x3fe2b9ddca191a18, 0x3fe9f2de7a59e6dc),
    (0x3fe404e73925ae72, 0x3fe2bd17a2d352a2, 0x3fe9f08a321a4970),
    (0x3fe467a0f5d708bb, 0x3fe30cc52ce2ca34, 0x3fe9b63f3ed37a88),
    (0x3fe46ee1254a28d3, 0x3fe31298201a234c, 0x3fe9b1ed94f96ae3),
    (0x3fe47d8a0d42ff43, 0x3fe31e5b97a907b4, 0x3fe9a92e14951d9f),
    (0x3fe4902ab94f0d9f, 0x3fe32d485b5c7634, 0x3fe99e08aa1bd200),
    (0x3fe4b5d26666534e, 0x3fe34b6002e05066, 0x3fe987661b453bf6),
    (0x3fe4c8ee18524e57, 0x3fe35a9b0e2d4c48, 0x3fe97bdc1aecef0b),
    (0x3fe4ccacafab2e26, 0x3fe35d9650d47852, 0x3fe979982a38e65a),
    (0x3fe4d9d2680b0cc2, 0x3fe3680c1b4d2be2, 0x3fe971a12b2a1331),
    (0x3fe4e3b99fce46ba, 0x3fe36feafb3307cc, 0x3fe96b9e7706b68a),
    (0x3fe4fc0ffbcb9086, 0x3fe3833aab643372, 0x3fe95cceb97b0157),
    (0x3fe541f54866c648, 0x3fe3ba71ce25a44c, 0x3fe931f392d890b9),
    (0x3fe54b73d86c3557, 0x3fe3c1eaa2530126, 0x3fe92c18001c97ed),
    (0x3fe5587a47919303, 0x3fe3cc27f87f73c0, 0x3fe9240b2a35726c),
    (0x3fe5ac9248ab872e, 0x3fe40df4c96d203e, 0x3fe8efadd3d9c393),
    (0x3fe5b5fe5e6d5f71, 0x3fe4154b8d87c83e, 0x3fe8e9c515a0e578),
    (0x3fe5ca3b012d3c89, 0x3fe42508d8995a84, 0x3fe8dd0cbdd915f1),
    (0x3fe60461763ff4ba, 0x3fe45216058dd05e, 0x3fe8b8486f01b833),
    (0x3fe624b4d615b936, 0x3fe46b045380ce4c, 0x3fe8a3b4cdd64caa),
    (0x3fe62d0aefffeb8e, 0x3fe4716edb2dbc5a, 0x3fe89e6246a2584b),
    (0x3fe6485ade81103b, 0x3fe4866a87d6714e, 0x3fe88ce68b402c6a),
    (0x3fe665e944771ee8, 0x3fe49d0eb3c0e512, 0x3fe879e6eeb847a7),
    (0x3fe683c67b7d1310, 0x3fe4b3dd63a2a992, 0x3fe8669f7362d308),
    (0x3fe6975ab90c6210, 0x3fe4c2c77aef1476, 0x3fe859f02fcc4235),
    (0x3fe6a15e6d1f774a, 0x3fe4ca65631206ec, 0x3fe8536fb9347012),
    (0x3fe6b9064c2f9c7f, 0x3fe4dc5b48730494, 0x3fe8440a8af90852),
    (0x3fe6bbae96d57743, 0x3fe4de5f14839064, 0x3fe8424efa509fcd),
    (0x3fe6c97a0ef6220f, 0x3fe4e8d253957136, 0x3fe8394db1e8417c),
    (0x3fe6d2c04173e0ff, 0x3fe4efd6b41adc62, 0x3fe8333d541bf28b),
    (0x3fe6e0144690d4bb, 0x3fe4f9e93e317458, 0x3fe82a82d2d94691),
    (0x3fe6efca074f9df8, 0x3fe505c3e5ed875e, 0x3fe82033a07de895),
    (0x3fe74086ff97e40f, 0x3fe5425fb51d4ee2, 0x3fe7eadc7fc05161),
    (0x3fe74b33ddb5d262, 0x3fe54a591f6cc7ee, 0x3fe7e3c394f17f8c),
    (0x3fe771a8abb1ef60, 0x3fe566ff64704356, 0x3fe7ca1c5a742721),
    (0x3fe77d112add9f60, 0x3fe56f793139e02a, 0x3fe7c2798c02f140),
    (0x3fe784ca4edace9a, 0x3fe5753492f71b5a, 0x3fe7bd4c73d60374),
    (0x3fe78c3c3346cbb1, 0x3fe57ab9e4e3f2ba, 0x3fe7b84dccdae2eb),
    (0x3fe78d8f5f0f9dd3, 0x3fe57bb5491a150e, 0x3fe7b76a1e21911c),
    (0x3fe7b5cb4aaa94ce, 0x3fe59975f39c0d60, 0x3fe79c54896c4f25),
    (0x3fe7b885a8b366f7, 0x3fe59b7926d95b1c, 0x3fe79a7d10a099cd),
    (0x3fe7c8119a0ef65e, 0x3fe5a6ee3d4d7b38, 0x3fe78ffae8d3b88a),
    (0x3fe7cf59e40ab175, 0x3fe5ac4a670a1cfe, 0x3fe78b0cdee73e0f),
    (0x3fe7d761c44cd0b8, 0x3fe5b23246e267c2, 0x3fe7859bb8fd0aa0),
    (0x3fe7e0485cda5e0a, 0x3fe5b8bc57520f96, 0x3fe77f91eb57c602),
    (0x3fe7e8fa2c30410b, 0x3fe5bf1e0056566a, 0x3fe779aa2bb9ae1b),
    (0x3fe7f7b7bb2af9d7, 0x3fe5c9ebfb3cb9ba, 0x3fe76fa33f692c2b),
    (0x3fe80a3e509a4165, 0x3fe5d779b07cfef6, 0x3fe763021aaa15da),
    (0x3fe81031fefa776e, 0x3fe5dbd2df5bbb74, 0x3fe75ef1b4d3fcec),
    (0x3fe815e630c155e1, 0x3fe5dffcf69f89ec, 0x3fe75b0be65866fb),
    (0x3fe81a9a35de940b, 0x3fe5e36b7f7b9fb0, 0x3fe757d496ba7a7d),
    (0x3fe81c701266de40, 0x3fe5e4c23666ed9c, 0x3fe7569328ee11c7),
    (0x3fe8703f5902a57c, 0x3fe62196932d246c, 0x3fe71cec37badc83),
    (0x3fe870e651f27250, 0x3fe6220f2bd69762, 0x3fe71c78bc18f77e),
    (0x3fe87417218e7110, 0x3fe6245cfba2df8e, 0x3fe71a43aa95d7bf),
    (0x3fe8961cd26f373e, 0x3fe63ce077000cd2, 0x3fe702ac0d77f127),
    (0x3fe89916a99f5243, 0x3fe63f0430a9430a, 0x3fe7009a86bb1204),
    (0x3fe8a0c90ba26bee, 0x3fe6448be77c8192, 0x3fe6fb4010dd72a8),
    (0x3fe8ad0011921058, 0x3fe64d5004174dbe, 0x3fe6f2be660b8eff),
    (0x3fe8c8348c3e764a, 0x3fe660ca659381ba, 0x3fe6dfc04ea86f13),
    (0x3fe8c9d8531caab9, 0x3fe661f66df34118, 0x3fe6de9ab88c0903),
    (0x3fe8ccf37f2291ff, 0x3fe6642e9cbe18a2, 0x3fe6dc6e6c6b1da1),
    (0x3fe8f01b1e081cde, 0x3fe67d3e7b9385d8, 0x3fe6c3c7597b1d5c),
    (0x3fe8f7edf95cd41e, 0x3fe682cea3e36814, 0x3fe6be4715a1db77),
    (0x3fe900a522c081c1, 0x3fe688ff8d68dd98, 0x3fe6b824b0eeaf45),
];

/// The platform `cos`.
#[allow(clippy::disallowed_methods)]
#[inline]
pub(crate) fn cos(x: f64) -> f64 {
    std::hint::black_box(x).cos()
}

/// The platform `sin`.
#[allow(clippy::disallowed_methods)]
#[inline]
pub(crate) fn sin(x: f64) -> f64 {
    std::hint::black_box(x).sin()
}

/// The platform `cosf`.
#[allow(clippy::disallowed_methods)]
#[inline]
pub(crate) fn cosf(x: f32) -> f32 {
    std::hint::black_box(x).cos()
}

/// `__sincos_stret(x)`: `(sin, cos)`.
pub(crate) fn sincos(x: f64) -> (f64, f64) {
    let bits = x.to_bits();
    match SINCOS_EXCEPTIONS.binary_search_by_key(&bits, |e| e.0) {
        Ok(i) => (
            f64::from_bits(SINCOS_EXCEPTIONS[i].1),
            f64::from_bits(SINCOS_EXCEPTIONS[i].2),
        ),
        Err(_) => (sin(x), cos(x)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exceptions_are_sorted_and_one_ulp_apart() {
        for w in SINCOS_EXCEPTIONS.windows(2) {
            assert!(w[0].0 < w[1].0);
        }
        for &(x, s, c) in SINCOS_EXCEPTIONS {
            let x = f64::from_bits(x);
            let ds = (sin(x).to_bits() as i64 - s as i64).abs();
            let dc = (cos(x).to_bits() as i64 - c as i64).abs();
            assert!(ds <= 1 && dc <= 1, "{x}");
        }
    }
}
