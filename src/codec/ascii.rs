//! Custom 7-bit ASCII graphic only encoding.

use std::mem;
use std::convert::Into;
use encoding::types::{Encoding, CodecError, StringWriter, RawDecoder, RawEncoder, ByteWriter};


/// A static castable reference to `AsciiGraphicEncoding`.
/// Usage: `let enc = ASCII_GRAPHIC as encoding::EncodingRef`.
pub const ASCII_GRAPHIC: &'static self::AsciiGraphicEncoding = &self::AsciiGraphicEncoding;

/// This custom encoding is derived from encoding::ASCIIEncoding.
/// The only difference is that it represents only graphic characters. All control characters
/// except tab and space are regarded as invalid.
#[derive(Clone, Copy)]
pub struct AsciiGraphicEncoding;

impl Encoding for AsciiGraphicEncoding {
    fn name(&self) -> &'static str { "ascii" }
    fn whatwg_name(&self) -> Option<&'static str> { None }
    fn raw_encoder(&self) -> Box<dyn RawEncoder> { AsciiGraphicEncoder::new() }
    fn raw_decoder(&self) -> Box<dyn RawDecoder> { AsciiGraphicDecoder::new() }
}


/// An encoder for ASCII.
#[derive(Clone, Copy)]
pub struct AsciiGraphicEncoder;


impl AsciiGraphicEncoder {
    pub fn new() -> Box<dyn RawEncoder> { Box::new(AsciiGraphicEncoder) }
}


impl RawEncoder for AsciiGraphicEncoder {
    fn from_self(&self) -> Box<dyn RawEncoder> { AsciiGraphicEncoder::new() }
    fn is_ascii_compatible(&self) -> bool { true }

    fn raw_feed(&mut self, input: &str, output: &mut dyn ByteWriter) -> (usize, Option<CodecError>) {
        output.writer_hint(input.len());

        // all non graphic is unrepresentable
        match input.as_bytes().iter().position(|&ch| ch >= 0x7F || (ch < 0x20) && (ch != 0x09) ) {
            Some(first_error) => {
                output.write_bytes(&input.as_bytes()[..first_error]);
                let len = input[first_error..].chars().next().unwrap().len_utf8();
                (first_error, Some(CodecError {
                    upto: (first_error + len) as isize, cause: "non-graphic character".into()
                }))
            }
            None => {
                output.write_bytes(input.as_bytes());
                (input.len(), None)
            }
        }
    }

    fn raw_finish(&mut self, _output: &mut dyn ByteWriter) -> Option<CodecError> {
        None
    }
}




/// A decoder for ASCII.
#[derive(Clone, Copy)]
pub struct AsciiGraphicDecoder;

impl AsciiGraphicDecoder {
    pub fn new() -> Box<dyn RawDecoder> { Box::new(AsciiGraphicDecoder) }
}

impl RawDecoder for AsciiGraphicDecoder {
    fn from_self(&self) -> Box<dyn RawDecoder> { AsciiGraphicDecoder::new() }
    fn is_ascii_compatible(&self) -> bool { true }

    fn raw_feed(&mut self, input: &[u8], output: &mut dyn StringWriter) -> (usize, Option<CodecError>) {
        output.writer_hint(input.len());

        fn write_ascii_bytes(output: &mut dyn StringWriter, buf: &[u8]) {
            output.write_str(unsafe {mem::transmute(buf)});
        }

        // all non graphic is error
        match input.iter().position(|&ch| ch >= 0x7F || (ch < 0x20) && (ch != 0x09) ) {
            Some(first_error) => {
                write_ascii_bytes(output, &input[..first_error]);
                (first_error, Some(CodecError {
                    upto: first_error as isize + 1, cause: "non graphic character".into()
                }))
            }
            None => {
                write_ascii_bytes(output, input);
                (input.len(), None)
            }
        }
    }

    fn raw_finish(&mut self, _output: &mut dyn StringWriter) -> Option<CodecError> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::ASCII_GRAPHIC;
    use encoding::EncodingRef;

    #[test]
    fn test_decoder() {
        let enc = ASCII_GRAPHIC as EncodingRef;
        let mut decoder = enc.raw_decoder();
        let mut ret = String::new();
        let input = "abc\u{3}\u{3}\u{3}\u{0}def\nghijk".as_bytes();
        let (offset, err) = decoder.raw_feed(&input[..], &mut ret);
        assert_eq!(ret, "abc");
        assert_eq!(offset, 3);
        assert_eq!(err.unwrap().upto, 4);
    }

}
