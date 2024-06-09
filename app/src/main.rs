/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use anyhow::Result;
use colored::*;

use wbproto_beautifier::{Arguments, Parser};
use wbproto_beautifier::beautify;

fn main() {
    let mut options = Arguments::parse();
    if options.files.is_empty() {
        options.inplace = false;
        beautify_file(None, &mut options).unwrap();
    } else {
        options.inplace |= options.files.len() > 1;
        let files = options.files.clone();
        for file in files {
            if options.inplace {
                print!("Formatting file {}: ", file);
            }
            let r = beautify_file(Some(file), &mut options);
            if let (false, Err(_)) = (options.inplace, &r) {
                r.unwrap()
            } else if let Err(err) = r {
                println!("{} ({})", "could not format".red(), err.to_string().red());
            }
        }
    }
}

fn beautify_file(file: Option<String>, options: &mut Arguments) -> Result<()> {
    let code = if let Some(file) = &file {
        let mut file = std::fs::File::open(file)?;
        read_to_string(&mut file, None)?.0 + "\n"
    } else {
        read_to_string(&mut std::io::stdin(), None)?.0 + "\n"
    };
    let result = beautify(code.as_str(), options)?;
    if options.inplace {
        print!("{}", "file formatted ".green());
        match std::fs::write(file.unwrap().as_str(), result.as_bytes()) {
            Ok(_) => println!("{}", "and overwritten.".green()),
            Err(_) => println!("{}", "but could not write back.".red()),
        }
    }
    Ok(())
}

/// Taken from helix-editor
/// Reads the first chunk from a Reader into the given buffer
/// and detects the encoding.
///
/// By default, the encoding of the text is auto-detected by
/// `encoding_rs` for_bom, and if it fails, from `chardetng`
/// crate which requires sample data from the reader.
/// As a manual override to this auto-detection is possible, the
/// same data is read into `buf` to ensure symmetry in the upcoming
/// loop.
fn read_and_detect_encoding<R: std::io::Read + ?Sized>(
    reader: &mut R,
    encoding: Option<&'static encoding_rs::Encoding>,
    buf: &mut [u8],
) -> Result<(
    &'static encoding_rs::Encoding,
    bool,
    encoding_rs::Decoder,
    usize,
)> {
    let read = reader.read(buf)?;
    let is_empty = read == 0;
    let (encoding, has_bom) = encoding
        .map(|encoding| (encoding, false))
        .or_else(|| {
            encoding_rs::Encoding::for_bom(buf).map(|(encoding, _bom_size)| (encoding, true))
        })
        .unwrap_or_else(|| {
            let mut encoding_detector = chardetng::EncodingDetector::new();
            encoding_detector.feed(buf, is_empty);
            (encoding_detector.guess(None, true), false)
        });
    let decoder = encoding.new_decoder();

    Ok((encoding, has_bom, decoder, read))
}

/// Taken from helix-editor
pub fn read_to_string<R: std::io::Read + ?Sized>(
    reader: &mut R,
    encoding: Option<&'static encoding_rs::Encoding>,
) -> Result<(String, &'static encoding_rs::Encoding, bool)> {
    let mut buf = [0u8; 0x2000];

    let (encoding, has_bom, mut decoder, read) =
        read_and_detect_encoding(reader, encoding, &mut buf)?;

    let mut slice = &buf[..read];
    let mut is_empty = read == 0;
    let mut buf_string = String::with_capacity(buf.len());

    loop {
        let mut total_read = 0usize;

        loop {
            let (result, read, ..) =
                decoder.decode_to_string(&slice[total_read..], &mut buf_string, is_empty);

            total_read += read;

            match result {
                encoding_rs::CoderResult::InputEmpty => {
                    debug_assert_eq!(slice.len(), total_read);
                    break;
                }
                encoding_rs::CoderResult::OutputFull => {
                    debug_assert!(slice.len() > total_read);
                    buf_string.reserve(buf.len())
                }
            }
        }

        if is_empty {
            debug_assert_eq!(reader.read(&mut buf)?, 0);
            break;
        }

        let read = reader.read(&mut buf)?;
        slice = &buf[..read];
        is_empty = read == 0;
    }
    Ok((buf_string, encoding, has_bom))
}
