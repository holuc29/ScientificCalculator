//! Programmer mode. The big table of cases below was produced by an independent
//! implementation of the semantics (a Python script with arbitrary-precision
//! integers), covering random expressions in all four word sizes with mixed
//! literal styles, precedence, shifts, rotates and shared variables.

use calc_core::programmer::parse_in_base;
use calc_core::{Base, CalcError, Calculator, ProgValue, WordSize};

fn word(bits: u32) -> WordSize {
    WordSize::from_bits(bits).unwrap()
}

/// A calculator with the integer variables the reference cases use.
fn setup() -> Calculator {
    let mut c = Calculator::new();
    c.set_variable("a", "12").unwrap();
    c.set_variable("b", "-3").unwrap();
    c.set_variable("c", "100").unwrap();
    c
}

fn run(c: &mut Calculator, bits: u32, input: &str) -> Result<ProgValue, CalcError> {
    c.programmer_eval(input, word(bits)).map(|o| o.value)
}

fn err(bits: u32, input: &str) -> CalcError {
    run(&mut setup(), bits, input).unwrap_err()
}

fn val(bits: u32, input: &str) -> String {
    let v = run(&mut setup(), bits, input).unwrap_or_else(|e| panic!("{input}: {e}"));
    format!("{}|{}", v.format(Base::Hex, false), v.signed())
}

pub const CASES: &[(u32, &str, &str, i64)] = &[
    (32, "194639051_7 & 0o1_7777777777 | a", "74038FFD", 1946390525),
    (32, "(~0b110011 + ror(211, 36)) % -c", "1D", 29),
    (32, "(0b10101110101101011111001101010001 xor 0o0) >>> 1 | 0 % 0X9ebcd5_4a % ~0X7fffffff", "575AF9A8", 1465579944),
    (32, "~2147483648", "7FFFFFFF", 2147483647),
    (16, "-(0b0 >>> 16)", "0", 0),
    (16, "rol(b, 19) mod ~(5136 nor c)", "FFEF", -17),
    (64, "(~0X2 nand 1844674407_3709551615) / (0b1000000000000000000000000000000000000000000000000000000000000000 >>> 26)", "0", 0),
    (8, "ror(128 / 0o377 >> 14 << 14, 9)", "0", 0),
    (32, "2 & -0o2", "2", 2),
    (64, "~((b xor 0b10 ^ 0b0) * ((9223372036854775808 | a) % (c | 0X7ffffffffffffff_f)))", "800000000000000B", -9223372036854775797),
    (8, "~ror(~b, 14)", "F7", -9),
    (8, "0b10000000 & b", "80", -128),
    (32, "0X2 >>> 31", "0", 0),
    (16, "-(0o104 nor 0X7fff)", "8000", -32768),
    (64, "-((~0X8000000000000000 nor ~0b111111111111111111111111111111111111111111111111111111111111111) % (~c - 0o1000000000000000000000))", "0", 0),
    (8, "-2", "FE", -2),
    (16, "~c", "FF9B", -101),
    (64, "ror(a, 56)", "C00", 3072),
    (32, "0o20000000000 xor c", "80000064", -2147483548),
    (8, "-(rol(0x5E, 2) ^ -c)", "1B", 27),
    (32, "~(0x1 >> 38) nand -0o1", "0", 0),
    (32, "a mod 0x5AE6240", "C", 12),
    (32, "(1 ^ 0x2) * (0b10000000000000_000000000000000000 * (214748364_7 xor b * 0o17777777777))", "0", 0),
    (32, "2 ^ a", "E", 14),
    (32, "c + (2147483_647 ^ c)", "7FFFFFFF", 2147483647),
    (64, "(0b0 nor a nor 0b1000111110100100000000101101111000010111111100011111011001000001) + (2 + c - 0X0)", "72", 114),
    (16, "-b", "3", 3),
    (64, "~0xE9 >>> 29 >> 0", "7FFFFFFFF", 34359738367),
    (16, "((a ^ 0Xc6 nor 0o100000 nand a) & ~-b) >>> 10", "0", 0),
    (64, "~0o1000000000000000000000", "7FFFFFFFFFFFFFFF", 9223372036854775807),
    (16, "-0o7_7777", "8001", -32767),
    (16, "~~0b0", "0", 0),
    (16, "0b11110111100001 % 0X8000 xor 206 << 24", "3DE1", 15841),
    (32, "0o377777777_77 mod c & (a ^ 0Xffffffff)", "FFFFFFF3", -13),
    (16, "c + ~rol(ror(5491, 7), 3)", "CF0C", -12532),
    (64, "~(2 mod c)", "FFFFFFFFFFFFFFFD", -3),
    (16, "c ^ 0x7FFF | rol(219, 14)", "FFBF", -65),
    (8, "198 << 12", "0", 0),
    (64, "rol(0o1, 72) << 52", "1000000000000000", 1152921504606846976),
    (8, "(1 mod 10 + c) mod 0b1_0000000 & ror(-b mod 127, 3)", "60", 96),
    (16, "~(c % ~b)", "FFFF", -1),
    (32, "rol(0x7829D07D, 9)", "53A0FAF0", 1403058928),
    (16, "--(0b1000000000000000 >>> 12)", "8", 8),
    (32, "-0o0", "0", 0),
    (64, "c", "64", 100),
    (16, "0o100000 & 0o177777", "8000", -32768),
    (64, "1844674407370955_1615 - (0o121620637736107_3270306 ^ 0b0)", "5C6F300877128F39", 6660595186819829561),
    (32, "rol(0b0, 21)", "0", 0),
    (64, "(0o2 & 0b1) >>> 42", "0", 0),
    (16, "-(0X1 >> 19 << 7)", "0", 0),
    (8, "~0x2", "FD", -3),
    (64, "-0b1111111111111111111111111111111111111111111111111111111111111111 * (a % c) >>> 36", "0", 0),
    (16, "-(a >>> 10 nor (0x0 nand 0x7FFF) - 0xE_E32 mod 0b1101101101111100)", "11CE", 4558),
    (32, "~(0o2336512745 * b) << 12", "FC1AE000", -65347584),
    (8, "~(0x37 % a)", "F8", -8),
    (8, "0X94 mod a", "0", 0),
    (64, "9223372036854775807 * 2 >> 50", "FFFFFFFFFFFFFFFF", -1),
    (64, "18446744073709551615 ^ 0o1", "FFFFFFFFFFFFFFFE", -2),
    (16, "rol(0, 11)", "0", 0),
    (64, "-9223372036854775807", "8000000000000001", -9223372036854775807),
    (64, "-180", "FFFFFFFFFFFFFF4C", -180),
    (32, "rol(-0X2, 37)", "FFFFFFDF", -33),
    (16, "~0xC5_B3 >>> 4", "3A4", 932),
    (8, "-~0", "1", 1),
    (32, "c", "64", 100),
    (8, "c << 9 | b", "FD", -3),
    (16, "ror(1, 18)", "4000", 16384),
    (8, "1 * (0xBC | b) + -ror(0b1011_110, 10) + rol(~128 << 5, 13)", "82", -126),
    (64, "~0b100001", "FFFFFFFFFFFFFFDE", -34),
    (8, "127 nand 0x80", "FF", -1),
    (16, "c << 25", "0", 0),
    (64, "9223372036854_775808 | 0x2", "8000000000000002", -9223372036854775806),
    (64, "c + (0o100000000000000_0000000 / 0X344ff457337c4fa4 xor 9223372036854775807 * 0X7fffffffffffffff) - b", "66", 102),
    (8, "(-128 * (0o200 nand b) xor 0X38) * rol(0X7f & 0o0 >>> 4, 10)", "0", 0),
    (8, "ror(c, 10) nand -0Xff & rol(c, 13)", "8C", -116),
    (64, "-b", "3", 3),
    (8, "~0b11111_11 & (b xor b)", "0", 0),
    (64, "ror(~a, 6) mod (-0b10 & c)", "FFFFFFFFFFFFFFE3", -29),
    (16, "32768 >> 7", "FF00", -256),
    (64, "0b10 * 162466_04533809352203 nand 0o1", "FFFFFFFFFFFFFFFF", -1),
    (64, "-(~ror(0o200, 32) + 0)", "8000000001", 549755813889),
    (32, "(0o104 & 0xAE20C71) / (0o131 % b)", "20", 32),
    (16, "0X0 nor 0", "FFFF", -1),
    (8, "~(-a >>> 0)", "B", 11),
    (64, "ror(0o0, 33) nand -9223372036854775807 / ~1", "FFFFFFFFFFFFFFFF", -1),
    (64, "(0x41 >>> 21) + ~0X2", "FFFFFFFFFFFFFFFD", -3),
    (64, "rol(b, 6)", "FFFFFFFFFFFFFF7F", -129),
    (8, "~0 >>> 15", "0", 0),
    (64, "0o2 | a", "E", 14),
    (32, "a / 0b10 >> 5", "0", 0),
    (32, "c", "64", 100),
    (64, "b", "FFFFFFFFFFFFFFFD", -3),
    (64, "-c", "FFFFFFFFFFFFFF9C", -100),
    (32, "~(0x80000000 | 230)", "7FFFFF19", 2147483417),
    (32, "0X0 + a + ror(0x2, 21)", "100C", 4108),
    (16, "0o311 >> 22", "0", 0),
    (8, "-0X74 + ~b", "8E", -114),
    (8, "a", "C", 12),
    (16, "32239 >>> 6 ^ (b xor ((0Xfff_f | 0o2) ^ 0o77777))", "7E0A", 32266),
    (8, "~127", "80", -128),
    (64, "((c & 0x1) >> 64) / ~rol(9223372036854775808, 60)", "0", 0),
    (16, "rol(b << 6, 15)", "7FA0", 32672),
    (64, "~a ^ b | (18446744073709551615 ^ 9223372036854775808) * (9223372036854775_808 * 0o16215_70144763134653161)", "800000000000000E", -9223372036854775794),
    (8, "0X0 nor -55", "36", 54),
    (8, "ror(0x80, 15) << 9", "0", 0),
    (8, "-rol(0x7F, 12)", "9", 9),
    (32, "2232881_07 * 0o65257633", "4C16809", 79783945),
    (16, "62814 xor b", "AA3", 2723),
    (16, "-ror(b, 21)", "1001", 4097),
    (16, "(42698 nand 0Xec & (46050 nor 0X8000)) - ~-62618", "577C", 22396),
    (32, "(ror(0o31274241213, 10) nor a) + (~0x56326ECF | -0X80000000)", "6DAD4D3", 115004627),
    (8, "-(b * 0b1110000 nor -b)", "B4", -76),
    (8, "(-100 & rol(0x7F, 15)) * -(0o177 >> 11)", "0", 0),
    (64, "0X221372451ef42881 << 46", "A20400000000000", 729653508378198016),
    (8, "0X1 mod 0b111111", "1", 1),
    (8, "0Xb7 nand 0o1 % 2", "FE", -2),
    (32, "b", "FFFFFFFD", -3),
    (64, "c & 18446744073709551615 xor b % 1 xor rol(b - 1900164317907344972, 55)", "D8F2D0A14F682134", -2813957425912602316),
    (16, "0x83_AF / ((1872 | 1) % (32768 mod c)) >> 13", "FFFF", -1),
    (16, "~(0X6d_a0 ^ 0b11110111111110)", "AFA1", -20575),
    (64, "(ror(1, 25) << 71) / ror(rol(a, 35), 62)", "0", 0),
    (16, "-(0X7fff ^ a)", "800D", -32755),
    (64, "0o0 << 9", "0", 0),
    (16, "~((0b1110001011111001 - 0xC135 | 0o0) << 0)", "DE3B", -8645),
    (8, "(0b1 ^ 0o2 | 1 ^ 0b10000000) xor 0o1 << 0", "82", -126),
    (16, "~19698", "B30D", -19699),
    (64, "~~9223372036854775808", "8000000000000000", -9223372036854775808),
    (8, "0Xf4 xor 0b1 * 205 xor -~0o2 xor c", "5E", 94),
    (32, "-(0b1 & b) << 0 nand ~-1", "FFFFFFFF", -1),
    (32, "a * (a % 0X7fffffff)", "90", 144),
    (64, "0Xb1e2c1d510f18a * b nor 0x0 >>> 37", "215A8457F32D49D", 150211179027027101),
    (16, "~-(0b1100111001001000 & b)", "CE47", -12729),
    (8, "ror(a, 5) + (rol(b, 9) + -34)", "39", 57),
    (8, "rol(~ror(0Xec, 16), 13) | -0o107 >> 3 nand ror(~0x1, 16)", "6B", 107),
    (8, "~0Xa", "F5", -11),
    (8, "-(0x7F | 0o0)", "81", -127),
    (64, "-ror(~c << 29, 28)", "FFFFFFF0000000CA", -68719476534),
    (16, "-0o0", "0", 0),
    (16, "c", "64", 100),
    (8, "-(0o301 / 0xFF)", "C1", -63),
    (8, "~2", "FD", -3),
    (8, "0Xec & (-0b1100001 ^ 0o53 & 0o301 | 0o264 nand 0b10 >>> 5)", "EC", -20),
    (64, "(b xor 1) - (0x8000000000000000 xor 0X5f7170d1076a0c61) nor ~ror(18, 71)", "400000000000000", 288230376151711744),
    (32, "~(a mod 1423022_427) % (4294_967295 * (34353_84713 / 0b1110110))", "FFFFFFF3", -13),
    (8, "2 + (a - 0xFF)", "F", 15),
    (64, "(0X82dda03598f148b3 ^ a) & -c", "82DDA03598F1489C", -9016874726866204516),
    (32, "(0b10100000 | 0x92B8A694) >> 39", "FFFFFFFF", -1),
    (64, "-11371506931912881394", "62304AB1BA70CB0E", 7075237141796670222),
    (16, "~-29354", "72A9", 29353),
    (64, "-~0b11010011101111010111110010100100100010001111100010_1010110110110", "69DEBE52447C55B7", 7628744079357793719),
    (16, "0x7FFF & (40965 nor 27971) / (46968 + 32767) ^ a >>> 8", "0", 0),
    (8, "0x80 | b & 0b10000000 >> 11", "FD", -3),
    (64, "~0Xfe554fa2f213e199 ^ 0x0", "1AAB05D0DEC1E66", 120102253790764646),
    (64, "~(0b1 >>> 52)", "FFFFFFFFFFFFFFFF", -1),
    (16, "c", "64", 100),
    (8, "b", "FD", -3),
    (64, "rol(a nor 0b0 ^ 0o16544503646656_53263405, 44) xor -~a >>> 20", "298F214DAF0B2515", 2994648894757610773),
    (8, "ror(2 | b, 2)", "FF", -1),
    (8, "62 * 0X7f", "C2", -62),
    (8, "a >> 3", "1", 1),
    (32, "~-0x2", "1", 1),
    (8, "-(13 | 0b1)", "F3", -13),
    (16, "0X2 - 65535", "3", 3),
    (64, "~0b1100_0100", "FFFFFFFFFFFFFF3B", -197),
    (16, "rol(-0o2 mod -0o2, 16) >>> 4", "0", 0),
    (64, "(rol(0x54, 37) >> 56 ^ ~ror(b, 37)) >>> 35", "0", 0),
    (8, "0b0 * 2", "0", 0),
    (8, "(0xFF + (ror(0, 11) xor -c)) % (~0x7F & (255 nand 0X1) xor ~0 nand -0x2)", "9B", -101),
    (8, "0xFB + b", "F8", -8),
    (8, "-b", "3", 3),
    (64, "~0Xffffffffffffffff >>> 50 nor 0x7FFFFFF_FFFFFFFFF * 0X2 >> 54", "0", 0),
    (8, "~(ror(2, 16) / c)", "FF", -1),
    (32, "--0b11100", "1C", 28),
    (64, "0b10100_11 << 19 xor ~(0b110111101000010101001001110111110000000100010011000011101100000 & 9370174154407148759)", "FDFF7F507DEE7BBF", -144256679365674049),
    (64, "-(2 - 190 + b)", "BF", 191),
    (64, "0 ^ b", "FFFFFFFFFFFFFFFD", -3),
    (8, "0b1_110100 >>> 4", "7", 7),
    (16, "~(~-42976 ^ 0X1)", "5821", 22561),
    (8, "0X80 * 0X1", "80", -128),
    (32, "0o37777777777 - 0x2 mod 0b1", "FFFFFFFF", -1),
    (32, "b", "FFFFFFFD", -3),
    (16, "~0o100000 - (0X2 | c) - (ror(0b10, 17) >> 19)", "7F99", 32665),
    (16, "0Xf1bb >> 22", "FFFF", -1),
    (8, "-~0X19 >> 7", "0", 0),
    (16, "rol(0X2 - -c, 1)", "CC", 204),
    (8, "255 mod 0b100_00000", "FF", -1),
    (16, "0b101101000100010 / 0b11011100 % rol(30285, 1)", "68", 104),
    (32, "~(c % 0X1 * (c ^ 0o6041435712))", "FFFFFFFF", -1),
    (64, "-~~1", "FFFFFFFFFFFFFFFF", -1),
    (16, "~(62460 - -(0b110_00 << 9))", "DC03", -9213),
    (16, "rol(0x0, 18)", "0", 0),
    (32, "c >>> 14", "0", 0),
    (16, "-(~-2 % (~0X396_9 & ~0X7f_ff))", "FFFF", -1),
    (16, "-~0b1111111111111111 ^ 0b101000_1100000111 - (32768 & 0X2 nor 0o0 - 0o340)", "A228", -24024),
    (16, "~b", "2", 2),
    (8, "0b10000000 | c", "E4", -28),
    (16, "-0o16476", "E2C2", -7486),
    (64, "-~c", "65", 101),
    (32, "-(0o17_777777777 nor 2182765408)", "0", 0),
    (32, "0b11111111111111111111111111111_111 + 2147483648 & -0X0", "0", 0),
    (64, "ror(0o1341156053704663504607 nand 98, 41) - (ror(0o777777777777777777777, 63) nand 0o713427137430242773_7 * 1)", "1CB8B97E20A2FDE", 129350498758832094),
    (16, "ror(0X42e8, 11)", "5D08", 23816),
    (32, "~2147483647", "80000000", -2147483648),
    (16, "0b1011101101011000 / 2", "DDAC", -8788),
    (32, "(b >>> 3) / rol(0b10, 23)", "1F", 31),
    (8, "c", "64", 100),
    (16, "~(0b1000000000000000 % 106 mod -65535)", "FFFF", -1),
    (64, "-0X8000000000_000000", "8000000000000000", -9223372036854775808),
    (8, "127 << 4", "F0", -16),
    (32, "~~(2147_483647 >>> 36) >> 12", "0", 0),
    (16, "(0o300 xor c) << 21", "0", 0),
    (64, "rol(a >>> 4, 47) / (-0x1 nand 0Xc50ac18ee815461e)", "0", 0),
    (32, "c * (0b100_00000000000000000000000000000 / c >> 22 | -b)", "FFFFFE0C", -500),
    (64, "c mod 0b1100011001000101010011110000010001011101000001001111010110000010", "64", 100),
    (16, "-(2 * 32767) / ((c ^ 65535) - -0o2)", "0", 0),
    (8, "0 | 0X1", "1", 1),
    (32, "-(0x7FFFFFFF >> 22 nand 0b11111111111111111111111111111111 >> 38 >>> 12)", "200", 512),
    (16, "a * (ror(32767, 21) nand -0x7FFF) >>> 11", "1F", 31),
    (8, "-~0x80 / (-0x2 >>> 11 | -~93)", "FF", -1),
    (8, "-(0o200 >> 10 >> 12)", "1", 1),
    (64, "0x2 nor 0b11100110111100101111000110101011001101000000101010100111010001", "C643439532FD562C", -4160407322675554772),
    (64, "b", "FFFFFFFFFFFFFFFD", -3),
    (16, "-(0Xa1 % a xor 0o7777_7 << 9)", "1FB", 507),
    (16, "(0b1000000000000000 nor 0X7_fff) >>> 4", "0", 0),
    (8, "0X7f >> 11", "0", 0),
    (16, "~(0X0 / 0x46 << 21)", "FFFF", -1),
    (64, "(-0 nor -0o335) * c", "55F0", 22000),
    (64, "rol(0X11206548ff1873ce, 48) >>> 17", "39E7089032A4", 63664443896484),
    (8, "0o1 >>> 7", "0", 0),
    (16, "61805 % 0o2", "FFFF", -1),
    (8, "-ror(~2 % rol(0b10000000, 12), 9)", "2", 2),
    (16, "~~0o1 << 19", "0", 0),
    (64, "9223372036854775808 << 34 | 1 >> 61", "0", 0),
    (64, "-(-(0b111111111111111111111111111111111111111111111111111111111111111 >>> 57) & (0b10111111101101110100110110100111000111_0100111111100010111001101 ^ 9223_372036854775807) % (c + b))", "FFFFFFFFFFFFFFFF", -1),
    (64, "0xFFFF_FFFFFFFFFFFF << 65 >>> 46", "0", 0),
    (32, "-4097681312", "BC25860", 197285984),
    (16, "c ^ a", "68", 104),
    (64, "rol(0b111111111111111111111111111111111111111111111111111111111111111 xor 0X3ee75f63676438db, 34)", "626F1C9104628272", 7092919347301220978),
    (16, "a", "C", 12),
    (8, "a mod c", "C", 12),
    (16, "a | b", "FFFD", -3),
    (16, "~c", "FF9B", -101),
    (8, "0x80 nand 0X1 mod b", "FF", -1),
    (8, "--c / 0o177", "0", 0),
    (32, "ror(0o37777777777 + b, 19) % 0b10", "FFFFFFFF", -1),
    (16, "b", "FFFD", -3),
    (8, "-~((1 nor 0o1) - (0o1 & 0b11111011))", "FE", -2),
    (64, "0o1777777777777777777777 xor 0x2B51DC2B99FDF6EB nor 0X1 mod 0x7FFFFFFFFFFFFFFF", "2B51DC2B99FDF6EA", 3121518096569726698),
    (8, "0b111_1111 xor 255", "80", -128),
    (64, "((8 - 0x7FFFFFFFFFFFFFFF) * 1761479232019_3446037 | -(0b10 >>> 50)) % 0X1", "0", 0),
    (64, "rol(0Xc9ef2be85fad589f, 38)", "EB5627F27BCAFA17", -1488958704386901481),
    (64, "(18446744073709551615 nand 0b1000000000000000000000000000000000000000000000000000000000000000 mod 0o731766602765610516371 ^ 0x8000000000000000) + (a >> 28)", "898127D051DD6306", -8538499642837605626),
    (8, "(128 & 0Xff) / 0o200", "1", 1),
    (8, "-128", "80", -128),
    (32, "ror(c, 34)", "19", 25),
    (32, "-(0b11111111111111111111111111111111 >>> 22) ^ a", "FFFFFC0D", -1011),
    (32, "(0o17777777777 mod a xor b) / (0b101001111100001110110101110011 - 0xD1 nor (c nor 191705269))", "0", 0),
    (8, "~0o64 * ~c + (0x0 xor 0x2 % 0b10000000)", "EB", -21),
    (16, "c", "64", 100),
    (16, "(0o2 xor 0x1 nor a) % 0x7FFF", "FFF0", -16),
    (32, "(0xFFF_FFFFF ^ 0X54 % 3738421544 xor -0o1 % (0x7FFFFFFF % 0o35305662300)) << 2", "150", 336),
    (64, "c % 0b111111111111111111111111111111111111111111111111111111111111111 xor 18446744073709551615", "FFFFFFFFFFFFFF9B", -101),
    (32, "(~(203 * 3412073629) & b) << 35", "0", 0),
    (8, "~(-154 nand (0x80 ^ 0X2) | 2 * b - ~229)", "2", 2),
    (64, "18446744073709551615 nand 0Xa7fc5c_8e730c15e6 ^ 0xFFFFFFFFFFFFFFFF % 1844674407_3709551615", "5803A3718CF3EA19", 6342092408359217689),
    (16, "~(0x1 nand 0X2322 xor 0X7fff & b)", "7FFD", 32765),
    (8, "ror(128, 17)", "40", 64),
    (16, "ror(2 nor 65535, 22)", "0", 0),
    (32, "ror(b, 29)", "FFFFFFEF", -17),
    (64, "~0o1", "FFFFFFFFFFFFFFFE", -2),
    (64, "c + (133 xor 0b0) >>> 44", "0", 0),
    (64, "1 >> 34", "0", 0),
    (32, "ror(0xD13712D1, 41)", "68E89B89", 1760074633),
    (32, "0b11001001110101101011011_00100010 >>> 9", "3275AD", 3306925),
    (8, "ror(~0o1, 16)", "FE", -2),
    (32, "0o0 / b * 0 mod 0b1011100 mod rol(1, 30)", "0", 0),
    (16, "rol(0x1 mod b * (0o160737 % b), 19)", "0", 0),
    (32, "b", "FFFFFFFD", -3),
    (32, "c", "64", 100),
    (32, "b", "FFFFFFFD", -3),
    (64, "~~(a ^ 0xD9)", "D5", 213),
    (8, "-~2", "3", 3),
    (64, "~9223372036854775807", "8000000000000000", -9223372036854775808),
    (64, "-(0X53 | 0x8000000000000000)", "7FFFFFFFFFFFFFAD", 9223372036854775725),
    (64, "--(0b1100111010000100101110011110101111111001100110010001000001010100 & (0xFFFFFFFFFFFFFFFF nand 0X80000_00000000000))", "4E84B9EBF9991054", 5657851455040196692),
    (8, "(b nor 0x7F / (0 nor a)) * (0 * (2 >> 9 & 255))", "0", 0),
    (64, "a nand 0b0", "FFFFFFFFFFFFFFFF", -1),
    (8, "~~(0X7f / 128 ^ 0)", "0", 0),
    (32, "0xDA & 0X2a4ae9", "C8", 200),
    (32, "(0 >>> 15) % -0x60D_A6500 - 0o2", "FFFFFFFE", -2),
    (16, "~ror(0o0, 25) nand (32768 ^ 0X8000) + -31291", "7A3A", 31290),
    (8, "rol(0o377, 17)", "FF", -1),
    (16, "(-(c xor 0xD4_7B) >>> 19) * 0X1544", "0", 0),
    (8, "rol(0o41, 4) ^ 2 + c", "74", 116),
    (16, "rol(ror(0b1111111111111111, 12), 19) nor -(0b1110011010000101 xor 0o0)", "0", 0),
    (32, "0X6439_2157 >> 28", "6", 6),
    (64, "-a", "FFFFFFFFFFFFFFF4", -12),
    (16, "-(0o2 >> 5)", "0", 0),
    (16, "~~0X132e >>> 23", "0", 0),
    (8, "0X40 - -(c << 14)", "40", 64),
    (64, "~b xor 33707_61184186091", "BF9B07F018EE9", 3370761184186089),
    (64, "-0o1000000000000000000000", "8000000000000000", -9223372036854775808),
    (8, "ror(128 * 245 >> 7 nand 0X2, 16)", "FD", -3),
    (32, "~~0b100000000_00000000000000000000000", "80000000", -2147483648),
    (16, "~(185 nor c)", "FD", 253),
    (8, "rol(rol(c & 162, 0), 3)", "1", 1),
    (64, "~0o1777777777777777777777", "0", 0),
    (16, "a", "C", 12),
    (32, "0xFF_FFFFFF * 0xAED831D7", "5127CE29", 1361563177),
    (64, "0o0 / 0x25", "0", 0),
    (8, "ror(0o264 << 0, 12)", "4B", 75),
    (8, "1 - 0x80", "81", -127),
    (8, "rol(0o24, 12)", "41", 65),
    (8, "b", "FD", -3),
    (64, "~0", "FFFFFFFFFFFFFFFF", -1),
    (64, "c", "64", 100),
    (16, "0b1000000000000000 >> 10", "FFE0", -32),
    (16, "((-32767 xor b) >>> 24) mod -a", "0", 0),
    (32, "0X0 >> 40", "0", 0),
    (8, "-0b0", "0", 0),
    (32, "~rol(0o354 & 0, 16)", "FFFFFFFF", -1),
    (8, "0b1_00100 % 0o377", "0", 0),
    (32, "~0o2", "FFFFFFFD", -3),
    (16, "c + 10298", "289E", 10398),
    (8, "rol(0b0 % a, 0)", "0", 0),
    (64, "-131 | 7360221847453795469 << 56", "FFFFFFFFFFFFFF7D", -131),
    (8, "0b10111001 nand 0X2", "FF", -1),
    (64, "(0X2 | 12966929048327925624) xor -0b11001000", "4C0C36BB76B85042", 5479815025381625922),
    (64, "~0X2 mod -b", "0", 0),
    (32, "rol(0, 37)", "0", 0),
    (64, "ror(-((b | 0o1_000000000000000000000) >> 71), 66)", "4000000000000000", 4611686018427387904),
    (16, "(c << 12) % (0o1 xor b)", "0", 0),
    (8, "b", "FD", -3),
    (64, "89769646016437_67269 + ((2 >> 51) - 0X1)", "7C9495BE423B95E4", 8976964601643767268),
    (32, "-0X0 % rol(21474836_47, 13)", "0", 0),
    (8, "c >> 8 << 5", "0", 0),
    (16, "rol(ror(0X80_00, 23), 10)", "4", 4),
    (64, "(71 * 0o1567572663206375132367 << 6) / b", "E279B5CC98269FEB", -2127469458625224725),
    (8, "1 mod 0X1", "0", 0),
    (64, "0b111111111111111111111111111111111111111111111111111111111111111 | 0X5f", "7FFFFFFFFFFFFFFF", 9223372036854775807),
    (64, "(c & 0X8000000000000000 mod 159) % ~c", "64", 100),
    (8, "-(1 xor a)", "F3", -13),
    (16, "~0b1100101", "FF9A", -102),
    (8, "0b10_1100 / a", "3", 3),
    (32, "ror((0b10 nor 0b10000000000000000000000000000000) / ~(0X80_000000 nand 0b10000000000000000000000000000000), 19)", "0", 0),
    (64, "0 & 0o77_7777777777777777777 + 0 ^ (0b1 | 92233720368547_75807) nand -0x1", "8000000000000000", -9223372036854775808),
    (16, "0x0 / 1", "0", 0),
    (8, "0X7f mod (0b11100 & -a / ~b)", "7", 7),
    (8, "(0b1111_111 nand c) / (a xor 0o177)", "0", 0),
    (32, "ror(-(0o20000000000 xor 0x0) | 0b10 / 0o20000000000 >> 9, 3)", "10000000", 268435456),
    (16, "c >> 4 << 23", "0", 0),
    (64, "~c << 19", "FFFFFFFFFCD80000", -52953088),
    (64, "-120", "FFFFFFFFFFFFFF88", -120),
    (16, "(0o270 ^ 65535) / (0b101111100_0111100 ^ 0xFA_39)", "0", 0),
    (64, "-0", "0", 0),
    (64, "18446744073709551615 / 0Xa4bf35b0f503830 - ~95 >>> 66", "0", 0),
    (16, "14090 % 0x34B9", "251", 593),
    (32, "-4294967295", "1", 1),
    (16, "-0x38F1", "C70F", -14577),
    (8, "c", "64", 100),
    (16, "(~0x92CE ^ -0b1) << 9", "9C00", -25600),
    (64, "ror(~(0x7FFFFFFFFFFFFFFF | 0o1), 30)", "200000000", 8589934592),
    (16, "~(0X7f_ff >> 17)", "FFFF", -1),
    (32, "0X0 << 25", "0", 0),
    (8, "0b1111111 xor b", "82", -126),
    (64, "0Xf6ec3a06f72cc08d - (0o1777777777777777777777 xor 0o77777777_7777777777777)", "76EC3A06F72CC08D", 8569287992570331277),
    (8, "~(-ror(1, 15) nor 0xFF)", "FF", -1),
    (32, "c", "64", 100),
    (64, "~(c >>> 28 xor 0xFFFFFFFFFFFFFFFF)", "0", 0),
    (16, "rol(~0b1000000000000000, 23)", "FFBF", -65),
    (8, "-~0o0", "1", 1),
    (32, "-2147483648", "80000000", -2147483648),
    (16, "32768 << 18", "0", 0),
    (8, "rol(128 nor 0b100_00000, 1) + -(255 >> 7) << 14", "0", 0),
    (16, "b", "FFFD", -3),
    (32, "0x1 xor ~(0X7fffffff xor b)", "7FFFFFFC", 2147483644),
    (64, "~(9223372036854775807 + 0X0)", "8000000000000000", -9223372036854775808),
    (32, "(((0o1 xor b) nand (0x7_FFFFFFF nor 0xFFFFFFFF)) << 22) - ~-(0b1000011011110010101101001_11101 >> 11)", "FFBBC86C", -4470676),
    (16, "~0b11011111 + -(c - a)", "FEC8", -312),
    (64, "~0x1", "FFFFFFFFFFFFFFFE", -2),
    (64, "c + a", "70", 112),
    (64, "0b1000000000000000000000000000000000000000000000000000000000000000 & ~c", "8000000000000000", -9223372036854775808),
    (16, "-0x383A", "C7C6", -14394),
    (32, "ror(0x542B73E4, 21) xor b >> 20", "A460DD5E", -1537155746),
    (16, "c >>> 10", "0", 0),
    (16, "0X7fff << 14", "C000", -16384),
    (8, "(0b10011100 & 0b1 & a % 0o200) >> 6", "0", 0),
    (8, "ror(1, 4) xor ~0x4F", "A0", -96),
    (16, "~(((32767 nor 42268) nand ~c) >>> 20)", "FFFF", -1),
    (16, "~b", "2", 2),
    (32, "~~b", "FFFFFFFD", -3),
    (64, "rol(121_46072293467066083, 8)", "8F8465F4518EE3A8", -8105241329345305688),
    (32, "b - 169 & b", "FFFFFF54", -172),
    (8, "-~c | rol(137, 12) / ~0x0", "6D", 109),
    (64, "0b11000111 | 2", "C7", 199),
    (8, "b xor c", "99", -103),
    (32, "0o112614_23010 | 0b11111111111111111111111111111111 mod ((131 ^ 0b1111111111111111111111111_1111111) mod rol(3481648107, 20))", "FFFFFFFF", -1),
    (32, "-33814145_77", "3673B54F", 913552719),
    (16, "-0b1", "FFFF", -1),
    (16, "0X44 * 0o76574", "54F0", 21744),
];

#[test]
fn matches_the_independent_reference() {
    let mut c = setup();
    for (bits, input, hex, signed) in CASES {
        let v = run(&mut c, *bits, input)
            .unwrap_or_else(|e| panic!("{bits}-bit `{input}`: {e}"));
        assert_eq!(
            (v.format(Base::Hex, false).as_str(), v.signed()),
            (*hex, *signed),
            "{bits}-bit `{input}`"
        );
    }
}

#[test]
fn literals_in_every_base() {
    assert_eq!(val(64, "0xFF"), "FF|255");
    assert_eq!(val(64, "0Xff"), "FF|255");
    assert_eq!(val(64, "0b1111_0000"), "F0|240");
    assert_eq!(val(64, "0o17"), "F|15");
    assert_eq!(val(64, "1_000_000"), "F4240|1000000");
    assert_eq!(val(64, "0xFFFFFFFFFFFFFFFF"), "FFFFFFFFFFFFFFFF|-1");
    assert_eq!(val(8, "0xFF"), "FF|-1");
    assert_eq!(val(8, "255"), "FF|-1");
}

#[test]
fn arithmetic_wraps_like_hardware() {
    assert_eq!(val(8, "0x7F + 1"), "80|-128");
    assert_eq!(val(8, "-128 - 1"), "7F|127");
    assert_eq!(val(8, "200 + 100"), "2C|44");
    assert_eq!(val(8, "16 * 16"), "0|0");
    assert_eq!(val(16, "-1"), "FFFF|-1");
    assert_eq!(val(32, "0 - 1"), "FFFFFFFF|-1");
    assert_eq!(val(64, "0x7FFFFFFFFFFFFFFF + 1"), "8000000000000000|-9223372036854775808");
    assert_eq!(val(8, "-(-128)"), "80|-128");
}

#[test]
fn division_truncates_and_mod_follows_the_dividend() {
    assert_eq!(val(32, "7 / 2"), "3|3");
    assert_eq!(val(32, "-7 / 2"), "FFFFFFFD|-3");
    assert_eq!(val(32, "7 / -2"), "FFFFFFFD|-3");
    assert_eq!(val(32, "-7 % 3"), "FFFFFFFF|-1");
    assert_eq!(val(32, "7 mod -3"), "1|1");
    assert_eq!(val(8, "-128 / -1"), "80|-128"); // overflow wraps
    assert!(matches!(err(32, "1 / 0"), CalcError::DivisionByZero));
    assert!(matches!(err(32, "5 % (3 - 3)"), CalcError::DivisionByZero));
}

#[test]
fn bitwise_operators_and_precedence() {
    assert_eq!(val(8, "0b1100 & 0b1010"), "8|8");
    assert_eq!(val(8, "0b1100 | 0b1010"), "E|14");
    assert_eq!(val(8, "0b1100 ^ 0b1010"), "6|6");
    assert_eq!(val(8, "0b1100 xor 0b1010"), "6|6");
    assert_eq!(val(8, "~0"), "FF|-1");
    assert_eq!(val(8, "0b1100 nand 0b1010"), "F7|-9");
    assert_eq!(val(8, "0b1100 nor 0b1010"), "F1|-15");
    // C precedence: shifts bind looser than +, tighter than &, & tighter than ^ than |
    assert_eq!(val(32, "1 << 2 + 1"), "8|8");
    assert_eq!(val(32, "6 & 3 | 8"), "A|10");
    assert_eq!(val(32, "6 | 3 & 8"), "6|6");
    assert_eq!(val(32, "1 | 2 ^ 3"), "1|1"); // 1 | (2^3)
    assert_eq!(val(32, "2 * 3 + 4 << 1"), "14|20");
    assert_eq!(val(32, "-2 + ~1"), "FFFFFFFC|-4");
    assert_eq!(val(32, "~-1"), "0|0");
}

#[test]
fn shifts_and_rotates() {
    assert_eq!(val(8, "1 << 7"), "80|-128");
    assert_eq!(val(8, "1 << 8"), "0|0");
    assert_eq!(val(8, "0x80 >> 1"), "C0|-64"); // arithmetic: sign is kept
    assert_eq!(val(8, "0x80 >>> 1"), "40|64"); // logical
    assert_eq!(val(8, "0x80 >> 20"), "FF|-1");
    assert_eq!(val(8, "0x40 >> 20"), "0|0");
    assert_eq!(val(8, "0x80 >>> 20"), "0|0");
    assert_eq!(val(8, "rol(0x81, 1)"), "3|3");
    assert_eq!(val(8, "ror(0x81, 1)"), "C0|-64");
    assert_eq!(val(8, "rol(0x12, 8)"), "12|18");
    assert_eq!(val(8, "rol(0x12, 12)"), "21|33");
    assert_eq!(val(32, "ror(1, 1)"), "80000000|-2147483648");
    assert_eq!(val(64, "rol(0x8000000000000001, 4)"), "18|24");
    assert!(matches!(err(32, "1 << -1"), CalcError::Domain(_)));
    assert!(matches!(err(32, "rol(1, -1)"), CalcError::Domain(_)));
    assert!(matches!(err(32, "rol(1)"), CalcError::Arity { .. }));
    assert!(matches!(err(32, "foo(1)"), CalcError::UnknownFunction(_)));
}

#[test]
fn literals_must_fit_and_be_whole() {
    assert!(matches!(err(8, "256"), CalcError::Domain(_)));
    assert!(matches!(err(8, "0x100"), CalcError::Domain(_)));
    assert!(matches!(err(16, "70000"), CalcError::Domain(_)));
    assert!(matches!(err(64, "0x10000000000000000"), CalcError::Domain(_)));
    assert!(matches!(err(64, "99999999999999999999999999999999999999999"), CalcError::Domain(_)));
    assert!(matches!(err(32, "2.5"), CalcError::Domain(_)));
    assert!(matches!(err(32, "1 + 0.5"), CalcError::Domain(_)));
    assert!(matches!(err(32, "0xG"), CalcError::Syntax(_)));
    assert!(matches!(err(32, "0b102"), CalcError::Syntax(_)));
    assert!(matches!(err(32, "0x"), CalcError::Syntax(_)));
}

#[test]
fn syntax_errors() {
    for bad in ["", "1 +", "(1", "1)", "1 2", "* 3", "1 << ", "<", "1 > 2", "1 @ 2", "()", "1 = 2"] {
        assert!(
            matches!(err(32, bad), CalcError::Syntax(_)),
            "{bad:?} gave {:?}",
            err(32, bad)
        );
    }
    let deep = format!("{}1{}", "(".repeat(500), ")".repeat(500));
    assert!(matches!(err(32, &deep), CalcError::Syntax(_)));
}

#[test]
fn variables_are_shared_and_must_be_whole_and_fit() {
    let mut c = setup();
    assert_eq!(run(&mut c, 32, "a + b").unwrap().signed(), 9);
    // an integer-valued result of scientific arithmetic is a whole number
    c.set_variable("four", "sqrt(16)").unwrap();
    c.set_variable("third", "0.1*30").unwrap(); // 3 up to binary noise
    assert_eq!(run(&mut c, 32, "four + third").unwrap().signed(), 7);

    c.set_variable("half", "5/2").unwrap();
    let e = run(&mut c, 32, "half + 1").unwrap_err();
    assert!(matches!(e, CalcError::Domain(ref m) if m.contains("not a whole number")), "{e}");
    assert!(matches!(run(&mut c, 32, "pi"), Err(CalcError::Domain(_))));
    assert!(matches!(run(&mut c, 32, "e"), Err(CalcError::Domain(_))));
    assert!(matches!(run(&mut c, 32, "nothing"), Err(CalcError::UnknownIdentifier(_))));

    c.set_variable("big", "2^70").unwrap();
    assert!(matches!(run(&mut c, 64, "big"), Err(CalcError::Domain(_))));
    c.set_variable("m", "1000").unwrap();
    assert!(matches!(run(&mut c, 8, "m"), Err(CalcError::Domain(_))));
    assert_eq!(run(&mut c, 16, "m").unwrap().signed(), 1000);
    c.set_variable("u", "255").unwrap();
    assert_eq!(run(&mut c, 8, "u").unwrap().signed(), -1); // fits as unsigned
    c.set_variable("lo", "-128").unwrap();
    assert_eq!(run(&mut c, 8, "lo").unwrap().signed(), -128);
    c.set_variable("lo2", "-129").unwrap();
    assert!(matches!(run(&mut c, 8, "lo2"), Err(CalcError::Domain(_))));
}

#[test]
fn assignment_stores_signed_values_and_leaves_the_session_alone() {
    let mut c = setup();
    let history = c.history().len();
    let out = c.programmer_eval("mask = 0xFF << 4", word(32)).unwrap();
    assert_eq!(out.assigned.as_deref(), Some("mask"));
    assert_eq!(out.value.signed(), 0xFF0);
    assert_eq!(c.history().len(), history);
    assert_eq!(c.eval_str("mask").unwrap(), "4080"); // visible to the calculator
    assert_eq!(run(&mut c, 32, "mask >> 4").unwrap().signed(), 255);

    // a negative result is stored as a negative number
    c.programmer_eval("neg = 0 - 5", word(8)).unwrap();
    assert_eq!(c.eval_str("neg").unwrap(), "-5");

    for bad in ["pi = 1", "sin = 1", "mod = 1", "rol = 1"] {
        let r = c.programmer_eval(bad, word(32));
        assert!(
            matches!(r, Err(CalcError::Assignment(_)) | Err(CalcError::Syntax(_))),
            "{bad}: {r:?}"
        );
    }
    // a failing right-hand side changes nothing
    assert!(c.programmer_eval("mask = 1 / 0", word(32)).is_err());
    assert_eq!(c.eval_str("mask").unwrap(), "4080");
}

#[test]
fn formatting_in_all_bases() {
    let v = ProgValue::from_i128(-2, word(16));
    assert_eq!(v.format(Base::Hex, false), "FFFE");
    assert_eq!(v.format(Base::Hex, true), "FFFE");
    assert_eq!(v.format(Base::Dec, false), "-2");
    assert_eq!(v.format(Base::Oct, false), "177776");
    assert_eq!(v.format(Base::Bin, false), "1111111111111110");
    assert_eq!(v.format(Base::Bin, true), "1111 1111 1111 1110");
    let big = ProgValue::from_pattern(0xDEADBEEF, word(32));
    assert_eq!(big.format(Base::Hex, true), "DEAD BEEF");
    assert_eq!(big.format(Base::Oct, true), "33 653 337 357");
    assert_eq!(ProgValue::zero(word(8)).format(Base::Bin, true), "0");
    assert_eq!(ProgValue::from_pattern(0x1F5, word(16)).format(Base::Hex, true), "1F5");
}

#[test]
fn bits_toggling_and_resizing() {
    let v = ProgValue::zero(word(8)).toggled(7).toggled(0);
    assert_eq!((v.pattern(), v.signed()), (0x81, -127));
    assert!(v.bit(7) && v.bit(0) && !v.bit(3) && !v.bit(8));
    assert_eq!(v.toggled(7).pattern(), 0x01);
    assert_eq!(v.toggled(9), v); // out of range: unchanged
    // widening keeps the signed value, narrowing keeps the low bits
    assert_eq!(v.resized(word(32)).signed(), -127);
    assert_eq!(v.resized(word(32)).pattern(), 0xFFFF_FF81);
    assert_eq!(ProgValue::from_pattern(0x1234, word(16)).resized(word(8)).pattern(), 0x34);
    assert_eq!(ProgValue::from_pattern(u64::MAX, word(64)).resized(word(8)).signed(), -1);
}

#[test]
fn parsing_the_base_boxes() {
    let p = |t: &str, b: Base, bits: u32| parse_in_base(t, b, word(bits));
    assert_eq!(p("ff", Base::Hex, 32).unwrap().unsigned(), 255);
    assert_eq!(p("0xFF", Base::Hex, 32).unwrap().unsigned(), 255);
    assert_eq!(p("DEAD BEEF", Base::Hex, 32).unwrap().unsigned(), 0xDEAD_BEEF);
    assert_eq!(p("1010 0101", Base::Bin, 8).unwrap().unsigned(), 0xA5);
    assert_eq!(p("17", Base::Oct, 8).unwrap().unsigned(), 15);
    assert_eq!(p("-5", Base::Dec, 8).unwrap().signed(), -5);
    assert_eq!(p("-128", Base::Dec, 8).unwrap().signed(), -128);
    assert_eq!(p("255", Base::Dec, 8).unwrap().signed(), -1);
    assert_eq!(p("", Base::Dec, 8).unwrap().signed(), 0);
    assert!(matches!(p("-129", Base::Dec, 8), Err(CalcError::Domain(_))));
    assert!(matches!(p("256", Base::Dec, 8), Err(CalcError::Domain(_))));
    assert!(matches!(p("1FF", Base::Hex, 8), Err(CalcError::Domain(_))));
    assert!(matches!(p("G", Base::Hex, 8), Err(CalcError::Syntax(_))));
    assert!(matches!(p("2", Base::Bin, 8), Err(CalcError::Syntax(_))));
    assert!(matches!(p("8", Base::Oct, 8), Err(CalcError::Syntax(_))));
    assert!(matches!(p("1.5", Base::Dec, 8), Err(CalcError::Syntax(_))));
    assert!(matches!(p("-1", Base::Hex, 8), Err(CalcError::Syntax(_))));
    assert!(matches!(p("-", Base::Dec, 8), Err(CalcError::Syntax(_))));
    // round trip through every base
    for bits in [8u32, 16, 32, 64] {
        for raw in [0u64, 1, 0x80, 0x7F, 0xFF, 0xABCD, u64::MAX] {
            let v = ProgValue::from_pattern(raw, word(bits));
            for base in Base::ALL {
                let back = p(&v.format(base, true), base, bits).unwrap();
                assert_eq!(back, v, "{bits} bits, {raw:#x}, {base:?}");
            }
        }
    }
}
