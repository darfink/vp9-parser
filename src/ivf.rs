//! IVF parsing.

use std::convert::TryInto;
use std::io::Read;

use crate::{Result, Vp9Error};

/// IVF is a simple container format for raw VP8/VP9 data.
///
/// Use the `iter()` to iterate over the frames.
#[derive(Debug, Clone)]
pub struct Ivf<R> {
    reader: R,
    header: IvfHeader,
}

impl<R: Read + Clone> Ivf<R> {
    /// Creates a new IVF using the given reader.
    pub fn new(mut reader: R) -> Result<Self> {
        let mut d = vec![0u8; std::mem::size_of::<IvfHeader>()];
        reader.read_exact(&mut d)?;

        let header = IvfHeader {
            signature: [d[0], d[1], d[2], d[3]],
            version: u16::from_le_bytes(d[4..=5].try_into().unwrap()),
            length: u16::from_le_bytes(d[6..=7].try_into().unwrap()),
            four_cc: [d[8], d[9], d[10], d[11]],
            width: u16::from_le_bytes(d[12..=13].try_into().unwrap()),
            height: u16::from_le_bytes(d[14..=15].try_into().unwrap()),
            frame_rate_rate: u32::from_le_bytes(d[16..=19].try_into().unwrap()),
            frame_rate_scale: u32::from_le_bytes(d[20..=23].try_into().unwrap()),
            frame_count: u32::from_le_bytes(d[24..=27].try_into().unwrap()),
            reserved: [d[28], d[29], d[30], d[31]],
        };

        if header.signature != [0x44, 0x4B, 0x49, 0x46] {
            return Err(Vp9Error::InvalidHeader("invalid signature".to_owned()));
        }

        if header.version != 0 {
            return Err(Vp9Error::InvalidHeader("invalid version".to_owned()));
        }

        if header.length != 32 {
            return Err(Vp9Error::InvalidHeader("invalid length".to_owned()));
        }

        if header.four_cc != [0x56, 0x50, 0x39, 0x30] {
            return Err(Vp9Error::InvalidHeader("invalid four_cc".to_owned()));
        }

        Ok(Self { reader, header })
    }

    /// The initial width of the video.
    pub fn width(&self) -> u16 {
        self.header.width
    }

    /// The initial height of the video.
    pub fn height(&self) -> u16 {
        self.header.height
    }

    /// The framerate of the video (frame_rate_rate * frame_rate_scale).
    ///
    /// Example:
    /// 24 fps with a scale of 1000 -> 24000
    pub fn frame_rate_rate(&self) -> u32 {
        self.header.frame_rate_rate
    }

    /// Divider of the seconds (integer math).
    pub fn frame_rate_scale(&self) -> u32 {
        self.header.frame_rate_scale
    }

    /// Number of frames stored inside the IVF.
    pub fn frame_count(&self) -> u32 {
        self.header.frame_count
    }

    /// Iterates over the frames inside the IVF.
    pub fn iter(&self) -> IvfIter<R> {
        IvfIter {
            reader: self.reader.clone(),
            size_buffer: [0u8; 4],
            timestamp_buffer: [0u8; 8],
            frame_count: self.frame_count(),
        }
    }
}

/// The IvF Header.
#[derive(Debug, Copy, Clone)]
struct IvfHeader {
    signature: [u8; 4],
    version: u16,
    length: u16,
    four_cc: [u8; 4],
    width: u16,
    height: u16,
    frame_rate_rate: u32,
    frame_rate_scale: u32,
    frame_count: u32,
    reserved: [u8; 4],
}

/// Frame inside an IVF.
pub struct IvfFrame {
    /// The timestamp of the frame.
    pub timestamp: u64,
    /// The data of the frame.
    pub data: Vec<u8>,
}

/// Iterates over the frames inside the IVF.
pub struct IvfIter<R> {
    reader: R,
    size_buffer: [u8; 4],
    timestamp_buffer: [u8; 8],
    frame_count: u32,
}

impl<R: Read> Iterator for IvfIter<R> {
    type Item = IvfFrame;

    fn next(&mut self) -> Option<IvfFrame> {
        if self.reader.read_exact(&mut self.size_buffer).is_err() {
            return None;
        }
        if self.reader.read_exact(&mut self.timestamp_buffer).is_err() {
            return None;
        }

        let size = u32::from_le_bytes(self.size_buffer);
        let timestamp = u64::from_le_bytes(self.timestamp_buffer);

        let mut data = vec![0u8; size as usize];

        if self.reader.read_exact(&mut data).is_err() {
            return None;
        }

        Some(IvfFrame { timestamp, data })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.frame_count as usize))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn parse_ivf_ffmpeg_header() {
        let header: Vec<u8> = vec![
            0x44, 0x4B, 0x49, 0x46, 0x00, 0x00, 0x20, 0x00, 0x56, 0x50, 0x39, 0x30, 0x00, 0x05,
            0xD0, 0x02, 0xE8, 0x03, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0xE9, 0x19, 0x09, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x8C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        let cursor = Cursor::new(header);

        let ivf = Ivf::new(cursor).unwrap();

        assert_eq!(ivf.width(), 1280);
        assert_eq!(ivf.height(), 720);
        assert_eq!(ivf.frame_rate_rate(), 1000); // FFMPEG doesn't seems to set the initial value properly.
        assert_eq!(ivf.frame_rate_scale(), 1); // FFMPEG doesn't seems to set the initial value properly.
        assert_eq!(ivf.frame_count(), 596457);
    }

    #[test]
    fn parse_ivf_header() {
        let header: Vec<u8> = vec![
            0x44, 0x4B, 0x49, 0x46, 0x00, 0x00, 0x20, 0x00, 0x56, 0x50, 0x39, 0x30, 0xB0, 0x00,
            0x90, 0x00, 0x30, 0x75, 0x00, 0x00, 0xE8, 0x03, 0x00, 0x00, 0x1D, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x98, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        let cursor = Cursor::new(header);

        let ivf = Ivf::new(cursor).unwrap();

        assert_eq!(ivf.width(), 176);
        assert_eq!(ivf.height(), 144);
        assert_eq!(ivf.frame_rate_rate(), 30000);
        assert_eq!(ivf.frame_rate_scale(), 1000);
        assert_eq!(ivf.frame_count(), 29);
    }

    #[test]
    fn iter_ivf() {
        let data: Vec<u8> = vec![
            0x44, 0x4B, 0x49, 0x46, 0x00, 0x00, 0x20, 0x00, 0x56, 0x50, 0x39, 0x30, 0xB0, 0x00,
            0x90, 0x00, 0x30, 0x75, 0x00, 0x00, 0xE8, 0x03, 0x00, 0x00, 0x1D, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x62, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0xF0, 0x08, 0x00, 0x9D, 0x01, 0x2A, 0xB0, 0x00, 0x90, 0x00, 0x0B, 0xC7,
            0x08, 0x85, 0x85, 0x88, 0x85, 0x84, 0x88, 0x74, 0x82, 0x00, 0x06, 0xA6, 0x5F, 0x5A,
            0xEA, 0x42, 0x91, 0xAE, 0xF7, 0xB6, 0xFB, 0x41, 0x22, 0x4F, 0xC7, 0xAC, 0xCB, 0xD6,
            0xBA, 0x0C, 0x17, 0x4D, 0x59, 0x0A, 0x3B, 0xD3, 0x6E, 0x61, 0xB6, 0x2F, 0xD5, 0xE4,
            0xA8, 0xF6, 0x14, 0x7B, 0x14, 0xCE, 0x81, 0xB7, 0x98, 0x21, 0x76, 0xDB, 0x4A, 0xC2,
            0x86, 0xD1, 0x69, 0xA4, 0x61, 0xA1, 0x8D, 0xD4, 0x84, 0x82, 0xA8, 0x7F, 0x00, 0x06,
            0x00, 0x00, 0xFE, 0xEC, 0x22, 0xCC, 0x00, 0x00, 0xEB, 0x0D, 0x20, 0x61, 0x77, 0x0F,
            0xE4, 0x00, 0x39, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x71, 0x04, 0x00, 0x2F, 0x13, 0xF8, 0x00, 0x18, 0x00, 0x1D, 0xCA, 0xB7, 0xF4, 0x00,
            0x01, 0x2F, 0x2D, 0x0E, 0x45, 0xE5, 0xA1, 0xC4, 0x97, 0xDF, 0xF9, 0x99, 0xE2, 0x46,
            0xA7, 0xB1, 0x51, 0x64, 0x42, 0x10, 0x6B, 0x3F, 0x0D, 0x00, 0x09, 0x00, 0x00, 0xFE,
            0xE6, 0xC8, 0x09, 0xFB, 0xFB, 0xA3, 0x38, 0x00, 0xA8, 0xE3, 0x00, 0xA2, 0x5A, 0x83,
            0x40, 0x45, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xD1,
            0x05, 0x00, 0x2F, 0x13, 0xF8, 0x00, 0x18, 0x6F, 0x9B, 0x9F, 0xE2, 0xEB, 0x0A, 0xD9,
            0x1D, 0x1C, 0x30, 0x01, 0x3D, 0x9A, 0x38, 0x3A, 0x2E, 0xE0, 0x02, 0xA3, 0xFB, 0x06,
            0xE1, 0xDC, 0x12, 0x83, 0x7E, 0x67, 0x40, 0x5C, 0x67, 0xF0, 0x2A, 0x87, 0x83, 0xD3,
            0xD8, 0xB2, 0x10, 0x18, 0x9C, 0xA8, 0x0A, 0x00, 0x00, 0xFE, 0xE6, 0xC7, 0xB2, 0x02,
            0x54, 0x76, 0x1E, 0x64, 0x00, 0xD1, 0x20, 0x96, 0x1D, 0x41, 0x98, 0xC0, 0x43, 0x00,
            0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xB1, 0x05, 0x00, 0x2F,
            0x13, 0xF8, 0x00, 0x18, 0x89, 0x19, 0x05, 0xAE, 0xAA, 0x1D, 0xF8, 0x00, 0xA5, 0xB5,
            0xBC, 0xD0, 0x9E, 0x13, 0xC2, 0xBF, 0x22, 0x8F, 0x1E, 0x00, 0x4B, 0x7F, 0x62, 0xAA,
            0x31, 0x6D, 0xB2, 0x38, 0x68, 0x92, 0x94, 0x80, 0xE0, 0x12, 0xBD, 0x4D, 0x52, 0xF3,
            0x50, 0x68, 0x09, 0x00, 0x00, 0xFE, 0x8A, 0x35, 0x89, 0xE1, 0x91, 0x58, 0xBA, 0x00,
            0x28, 0xD4, 0x44, 0xC3, 0xC8, 0x56, 0xC0, 0x5D, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0xF1, 0x07, 0x00, 0x2F, 0x13, 0xB0, 0x00, 0x18, 0x54,
            0x26, 0xD7, 0xB8, 0x2B, 0x68, 0x62, 0xF0, 0xC0, 0x0B, 0x0C, 0x4B, 0x2C, 0xA4, 0x5F,
            0x60, 0x61, 0x76, 0x25, 0x06, 0xC2, 0xDB, 0xB9, 0x6D, 0x70, 0x00, 0x3E, 0x73, 0x07,
            0x93, 0x7F, 0xC5, 0xDB, 0xC2, 0xA5, 0x35, 0x59, 0x52, 0x66, 0x5F, 0xEB, 0x0A, 0xB5,
            0x6E, 0xD3, 0xC8, 0x0C, 0xF3, 0x94, 0x1B, 0x07, 0x2A, 0xBF, 0xC5, 0x8F, 0x94, 0xBD,
            0x18, 0x0E, 0x00, 0x00, 0xFE, 0xA0, 0xAA, 0xF5, 0x13, 0xFE, 0xB6, 0x60, 0xE2, 0xF5,
            0xA2, 0xF4, 0x70, 0x00, 0x7E, 0xB4, 0x1F, 0x62, 0x2D, 0x60, 0xB4, 0x80, 0x11, 0x00,
            0x3B, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xB1, 0x04,
            0x00, 0x2F, 0x13, 0xF8, 0x00, 0x18, 0x00, 0x20, 0xFA, 0xB7, 0xF4, 0x00, 0x02, 0xDE,
            0xC8, 0x63, 0x04, 0x31, 0x82, 0x19, 0x1C, 0x5A, 0xEF, 0x70, 0x99, 0x27, 0x5E, 0x2C,
            0xE9, 0x99, 0x41, 0x63, 0xED, 0x1F, 0xFA, 0x89, 0x04, 0x00, 0x09, 0x00, 0x00, 0xFE,
            0x8A, 0x5D, 0xCE, 0xA7, 0x3D, 0x38, 0x88, 0xE0, 0x48, 0x93, 0x47, 0x06, 0x83, 0x98,
            0x00, 0x4A, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xF1,
            0x05, 0x00, 0x2F, 0x13, 0xBC, 0x00, 0x18, 0x00, 0x23, 0xE4, 0x1F, 0xF4, 0x00, 0x08,
            0x12, 0xBB, 0xB0, 0x2F, 0x6E, 0xA2, 0xF7, 0x5A, 0xCA, 0x97, 0xC3, 0xA2, 0x64, 0x6B,
            0xA2, 0xAF, 0x09, 0x5B, 0x16, 0xC9, 0x14, 0xD5, 0x1F, 0x9D, 0x85, 0x6A, 0xF5, 0xE2,
            0x34, 0x6B, 0x65, 0xCA, 0x69, 0x32, 0x00, 0x0B, 0x00, 0x00, 0xFE, 0x9D, 0x42, 0xFB,
            0xF5, 0x2F, 0x33, 0x34, 0x38, 0x2E, 0x80, 0x3B, 0x93, 0xDE, 0xCF, 0xE1, 0xC6, 0x24,
            0xFE, 0xE4, 0x38, 0x46, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x51, 0x06, 0x00, 0x2F, 0x13, 0xF8, 0x00, 0x18, 0x00, 0x1F, 0x34, 0x67, 0xF4,
            0x00, 0x03, 0x1C, 0x7D, 0x12, 0xFB, 0x7D, 0x14, 0xD9, 0xB0, 0x59, 0x37, 0xAD, 0x91,
            0x74, 0xDC, 0x3A, 0x1E, 0x02, 0x2E, 0x0C, 0x01, 0x9C, 0x26, 0x41, 0x93, 0x4C, 0x89,
            0x7C, 0x89, 0x7D, 0xBE, 0x68, 0xD1, 0x02, 0xEE, 0xEB, 0x1D, 0xF5, 0x00, 0x08, 0x00,
            0x00, 0xFE, 0x8A, 0x5D, 0xD8, 0x9C, 0xA9, 0xC4, 0x00, 0x48, 0xC4, 0xAE, 0xE9, 0x63,
            0x00, 0x4D, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x91,
            0x06, 0x00, 0x2F, 0x13, 0xC4, 0x00, 0x18, 0x56, 0x7A, 0x82, 0x51, 0x4F, 0x8F, 0x34,
            0x00, 0xC4, 0x36, 0x8D, 0x88, 0x6D, 0x1B, 0xF2, 0xCB, 0xAC, 0x7E, 0xA8, 0x04, 0xE4,
            0x72, 0x2B, 0xBC, 0x7F, 0x1C, 0xE0, 0x5C, 0x23, 0x87, 0xA8, 0x44, 0x33, 0x46, 0x26,
            0x57, 0xCC, 0x5D, 0x8B, 0xAC, 0x7F, 0x47, 0xEB, 0xE2, 0xAE, 0x4E, 0x00, 0x0A, 0x00,
            0x00, 0xFE, 0x9B, 0x32, 0xDB, 0x8E, 0xDB, 0x41, 0xDF, 0x22, 0x00, 0x88, 0xB4, 0x7A,
            0x9C, 0x09, 0x8B, 0x51, 0xDA, 0xC0, 0x43, 0x00, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x71, 0x05, 0x00, 0x2F, 0x13, 0xF8, 0x00, 0x18, 0x59, 0x08,
            0x83, 0x91, 0xD1, 0x1E, 0xD8, 0x00, 0xF5, 0x1A, 0xCE, 0xEF, 0x51, 0xAC, 0xEF, 0x12,
            0x5A, 0x6E, 0x76, 0x76, 0x76, 0x76, 0x3A, 0x28, 0x44, 0x5C, 0x87, 0xE2, 0x61, 0x54,
            0x38, 0x6F, 0xFD, 0xC5, 0x2C, 0x89, 0xF7, 0x40, 0x0B, 0x00, 0x00, 0xFE, 0x8A, 0x5F,
            0xCA, 0x08, 0xD0, 0xBF, 0x58, 0x17, 0xDF, 0x00, 0x34, 0xF8, 0x3A, 0xCF, 0x65, 0x2F,
            0x00, 0x4E, 0x00, 0x00, 0x00, 0x0A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x31,
            0x07, 0x00, 0x24, 0x13, 0xC8, 0x00, 0x18, 0x42, 0xE9, 0x04, 0x51, 0xB1, 0x3E, 0x50,
            0x00, 0x88, 0xB4, 0x02, 0x6D, 0xF1, 0xE7, 0x9B, 0x31, 0x23, 0x97, 0xDC, 0xDF, 0xB3,
            0xD8, 0x6A, 0x3A, 0xDC, 0xDB, 0xE2, 0x52, 0x99, 0xD2, 0x14, 0x51, 0x8A, 0x80, 0x41,
            0xA7, 0x61, 0x20, 0x13, 0x66, 0xA8, 0x84, 0x38, 0x98, 0xA2, 0x1C, 0xB5, 0x76, 0x22,
            0x87, 0x2D, 0xE8, 0x07, 0x00, 0x00, 0xFE, 0x98, 0xF5, 0xB4, 0x02, 0x59, 0xF0, 0x32,
            0x67, 0xA8, 0x7B, 0x89, 0x41, 0x5F, 0x30, 0x45, 0x00, 0x00, 0x00, 0x0B, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x71, 0x05, 0x00, 0x2D, 0x13, 0xF8, 0x00, 0x18, 0x00,
            0x25, 0x1B, 0xEF, 0xF4, 0x00, 0x09, 0x73, 0x8F, 0x9E, 0x61, 0x51, 0x9D, 0xF7, 0xB6,
            0x74, 0xC9, 0x3C, 0x82, 0xDE, 0x83, 0xE6, 0x78, 0x1E, 0xCD, 0x0C, 0x88, 0x9E, 0x55,
            0x08, 0x83, 0xD3, 0xA3, 0xFC, 0x3A, 0x0E, 0x64, 0xC0, 0x0B, 0x00, 0x00, 0xFE, 0x8A,
            0x64, 0x98, 0x14, 0xE4, 0x05, 0xBF, 0xBF, 0xF8, 0x00, 0x32, 0x4C, 0x8F, 0xAF, 0xFC,
            0x59, 0x0C, 0x14, 0x80, 0x52, 0x00, 0x00, 0x00, 0x0C, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x71, 0x06, 0x00, 0x2D, 0x13, 0xD4, 0x00, 0x18, 0x00, 0x25, 0x94, 0xDF,
            0xF4, 0x00, 0x0C, 0x2D, 0xF8, 0x8D, 0xF8, 0x8D, 0xF8, 0x50, 0xE2, 0x85, 0x68, 0x73,
            0x4B, 0xD8, 0x0E, 0xD1, 0x79, 0x5C, 0x28, 0xE8, 0xFD, 0x66, 0xB7, 0x4D, 0xCC, 0x91,
            0x72, 0x98, 0xBF, 0xE6, 0x4A, 0xED, 0xF1, 0x99, 0xBD, 0x6D, 0x13, 0xE3, 0xAC, 0x00,
            0x0D, 0x00, 0x00, 0xFE, 0x96, 0x81, 0xAC, 0x7D, 0xC5, 0xC1, 0xC2, 0x20, 0x85, 0x57,
            0x2A, 0x00, 0x38, 0x23, 0xF8, 0x4A, 0x90, 0x9E, 0x7E, 0x71, 0x01, 0xAE, 0x08, 0x00,
            0x45, 0x00, 0x00, 0x00, 0x0D, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x51, 0x06,
            0x00, 0x2D, 0x13, 0xF8, 0x00, 0x18, 0x00, 0x20, 0x6D, 0x27, 0xF4, 0x00, 0x05, 0x01,
            0xF3, 0x43, 0xAA, 0xED, 0x27, 0x82, 0x2C, 0x46, 0x6A, 0xB7, 0x52, 0x45, 0x6E, 0x6E,
            0x23, 0xF1, 0xF5, 0x20, 0xE7, 0x91, 0x7B, 0x74, 0xB1, 0x5A, 0x76, 0xE3, 0x31, 0xBF,
            0xA2, 0xC2, 0x7E, 0xB5, 0x0B, 0x77, 0x6A, 0xC0, 0x00, 0x07, 0x00, 0x00, 0xFE, 0x8A,
            0x64, 0x3D, 0xA3, 0x59, 0xA0, 0x95, 0xCD, 0xD4, 0xC0, 0xDF, 0x20, 0x4D, 0x00, 0x00,
            0x00, 0x0E, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xD1, 0x05, 0x00, 0x2D, 0x13,
            0xDC, 0x00, 0x18, 0x00, 0x25, 0x4C, 0x97, 0xF4, 0x00, 0x0B, 0x05, 0x26, 0x3D, 0x7D,
            0x38, 0xD8, 0x5B, 0xFE, 0x32, 0x5E, 0x74, 0xD4, 0x40, 0xB5, 0x2B, 0xA6, 0x65, 0x98,
            0x45, 0xE4, 0xF8, 0x88, 0x92, 0x49, 0x5A, 0xCB, 0xF6, 0x77, 0x94, 0x20, 0x86, 0xBA,
            0x37, 0x00, 0x0E, 0x00, 0x00, 0xFE, 0x93, 0xD4, 0xC4, 0x2D, 0x8C, 0x35, 0xD6, 0xC2,
            0x6E, 0xEB, 0x00, 0x34, 0x00, 0x42, 0x46, 0x3A, 0x22, 0x42, 0x7A, 0xA9, 0x01, 0x59,
            0xAC, 0x00, 0x43, 0x00, 0x00, 0x00, 0x0F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x31, 0x05, 0x00, 0x2D, 0x13, 0xF8, 0x00, 0x18, 0x43, 0xF7, 0x05, 0xB0, 0x41, 0xDE,
            0x50, 0x01, 0x84, 0x02, 0x3A, 0x02, 0x28, 0xB0, 0x5B, 0x5C, 0xC8, 0x32, 0x5A, 0xEE,
            0x67, 0x88, 0x78, 0x26, 0x86, 0xAB, 0x60, 0x8A, 0x95, 0x82, 0xD6, 0x96, 0xAC, 0xE4,
            0xD0, 0xC0, 0x0B, 0x00, 0x00, 0xFE, 0x8A, 0x67, 0x94, 0xAD, 0x2B, 0x08, 0x21, 0xCC,
            0x63, 0xE0, 0x3B, 0x93, 0x80, 0x20, 0x8F, 0x85, 0xFC, 0x7D, 0x38, 0x4A, 0x00, 0x00,
            0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xF1, 0x05, 0x00, 0x2D, 0x13,
            0xF8, 0x14, 0x60, 0x00, 0x98, 0x35, 0x5F, 0xD0, 0x00, 0x3B, 0x02, 0x23, 0xBF, 0xAE,
            0x09, 0xD8, 0x8C, 0xD1, 0xCC, 0x2F, 0x69, 0xE9, 0xA7, 0x36, 0xE5, 0x6B, 0x2A, 0x7F,
            0xDC, 0x57, 0x35, 0xD2, 0xB7, 0xF6, 0x5F, 0xBC, 0xBD, 0xEF, 0x9F, 0xC8, 0x02, 0xB3,
            0x12, 0x11, 0xFE, 0x0A, 0x00, 0x00, 0xFE, 0x8A, 0x65, 0x7B, 0x08, 0x69, 0xD3, 0x20,
            0x00, 0xF0, 0x14, 0x7D, 0xB2, 0x75, 0x56, 0xEC, 0x9B, 0xFB, 0xBD, 0x23, 0x00, 0x52,
            0x00, 0x00, 0x00, 0x11, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x31, 0x07, 0x00,
            0x22, 0x13, 0xF8, 0x00, 0x18, 0x00, 0x24, 0x8E, 0x17, 0xF4, 0x00, 0x0D, 0x75, 0xFA,
            0xA6, 0x16, 0xE9, 0x1E, 0x71, 0x44, 0xDE, 0xC0, 0x78, 0x5B, 0x3B, 0xC7, 0x56, 0x1C,
            0x97, 0xEB, 0x4B, 0x23, 0xD9, 0x1B, 0x61, 0xA5, 0x87, 0x3C, 0x04, 0x8D, 0x86, 0xA0,
            0x78, 0x80, 0x29, 0x32, 0xC0, 0x46, 0x8C, 0xC8, 0xDA, 0xE9, 0x9D, 0x8E, 0x39, 0xF7,
            0xD0, 0x0A, 0x00, 0x00, 0xFE, 0x8A, 0x5D, 0x7C, 0x9F, 0x53, 0x9E, 0x4E, 0x60, 0x00,
            0x88, 0xB4, 0x46, 0x8F, 0x62, 0x66, 0xE1, 0x06, 0xC0, 0x78, 0x00, 0x00, 0x00, 0x12,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xF0, 0x0A, 0x00, 0x9D, 0x01, 0x2A, 0xB0,
            0x00, 0x90, 0x00, 0x0B, 0xC7, 0x08, 0x85, 0x85, 0x88, 0x85, 0x84, 0x88, 0x7F, 0x02,
            0x22, 0x46, 0x41, 0x31, 0xBE, 0x71, 0xBB, 0x79, 0x3F, 0x11, 0x19, 0xBA, 0xF2, 0x2D,
            0x21, 0xDC, 0x87, 0x72, 0x14, 0x39, 0x81, 0x98, 0x97, 0x6D, 0x0E, 0x98, 0x8A, 0xC4,
            0x20, 0xD2, 0xEB, 0x8B, 0xBA, 0x4F, 0xF6, 0xDE, 0x2B, 0x47, 0x6B, 0x49, 0xE2, 0xEE,
            0x93, 0xFD, 0xB7, 0x9D, 0xC7, 0x2C, 0xC4, 0x6A, 0x08, 0x7C, 0x42, 0x91, 0x2F, 0xD5,
            0xFE, 0x7A, 0x2C, 0x30, 0x9C, 0x98, 0x98, 0xF6, 0xB8, 0x82, 0x7E, 0x20, 0x9C, 0x3F,
            0xF5, 0xE3, 0xEE, 0x00, 0xC6, 0x00, 0x09, 0x00, 0x00, 0xFE, 0x8A, 0x5B, 0x97, 0x22,
            0x2F, 0x26, 0x03, 0xC0, 0x5C, 0x55, 0xE5, 0x4C, 0xDA, 0xF0, 0xE7, 0x7C, 0xED, 0x7C,
            0xE0, 0x4B, 0x00, 0x00, 0x00, 0x13, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x51,
            0x05, 0x00, 0x2F, 0x13, 0xF8, 0x00, 0x18, 0x6F, 0x9B, 0xF2, 0x1F, 0xFE, 0x7D, 0xE1,
            0x90, 0x15, 0x69, 0x87, 0x00, 0x63, 0x13, 0x18, 0x98, 0xC4, 0xC6, 0x26, 0x31, 0x34,
            0x9D, 0x72, 0xC7, 0x19, 0xBC, 0x5F, 0x32, 0x22, 0x72, 0x62, 0xDB, 0xC6, 0x37, 0x39,
            0xC6, 0xD0, 0x0F, 0x00, 0x00, 0xFE, 0x8A, 0x5D, 0xD9, 0x8E, 0x72, 0x7A, 0x02, 0x13,
            0x07, 0x12, 0xC2, 0x54, 0xF2, 0x00, 0x34, 0xD1, 0xF1, 0x0A, 0xF3, 0x46, 0x11, 0x85,
            0xDD, 0xA0, 0x3B, 0x80, 0x44, 0x00, 0x00, 0x00, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x31, 0x05, 0x00, 0x2F, 0x13, 0xF8, 0x00, 0x18, 0x56, 0x7A, 0x82, 0xA3,
            0xFA, 0xF7, 0x34, 0x00, 0xCC, 0xC1, 0x91, 0xF4, 0x99, 0x83, 0x23, 0xE6, 0x84, 0x5C,
            0xBD, 0xF8, 0xAD, 0xE1, 0xAB, 0x00, 0xF0, 0xCA, 0x04, 0x82, 0x29, 0xB2, 0xDA, 0xC3,
            0xB6, 0x9E, 0x2C, 0x38, 0x0B, 0x00, 0x00, 0xFE, 0x8A, 0x5E, 0x67, 0x54, 0x04, 0x82,
            0x38, 0x18, 0x05, 0x00, 0x39, 0x7D, 0x53, 0x81, 0x80, 0x9E, 0x86, 0x00, 0xB0, 0x00,
            0x44, 0x00, 0x00, 0x00, 0x15, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xB1, 0x04,
            0x00, 0x2F, 0x13, 0xF8, 0x00, 0x18, 0x56, 0x82, 0x81, 0x2B, 0x0B, 0x7B, 0xAA, 0x00,
            0x51, 0x2F, 0x83, 0x77, 0xC1, 0x25, 0x51, 0x2F, 0x80, 0x65, 0x02, 0xC4, 0x75, 0x93,
            0x7E, 0x5A, 0xB6, 0x3A, 0x18, 0x39, 0xD0, 0xDD, 0xE3, 0x00, 0x0A, 0x00, 0x00, 0xFE,
            0x8A, 0x57, 0x42, 0x16, 0x36, 0xDC, 0xE6, 0xDB, 0x60, 0x7E, 0xB4, 0x86, 0xB5, 0xAA,
            0xD0, 0xFE, 0x76, 0xDE, 0xB6, 0xDD, 0x05, 0xEE, 0x68, 0x00, 0x49, 0x00, 0x00, 0x00,
            0x16, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xB1, 0x05, 0x00, 0x19, 0x13, 0xF8,
            0x00, 0x18, 0x00, 0x26, 0xCC, 0x1F, 0xF4, 0x00, 0x0C, 0x3A, 0x9B, 0x49, 0x37, 0x36,
            0x92, 0x6E, 0xE0, 0xDB, 0xA3, 0x95, 0xE4, 0x8F, 0xA8, 0x7B, 0xE7, 0xDD, 0x21, 0x9A,
            0xB9, 0x8C, 0xD5, 0x76, 0x48, 0x33, 0x69, 0x45, 0x3D, 0xC0, 0x36, 0xF6, 0x98, 0x00,
            0x0B, 0x00, 0x00, 0xFE, 0x8A, 0x5D, 0xD2, 0xC1, 0x5B, 0x70, 0x07, 0xDE, 0x78, 0x00,
            0x33, 0x2A, 0xEF, 0x0C, 0x06, 0x7B, 0x71, 0xF3, 0x9F, 0x87, 0x90, 0x4A, 0x00, 0x00,
            0x00, 0x17, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x91, 0x05, 0x00, 0x19, 0x13,
            0xF8, 0x00, 0x18, 0x00, 0x28, 0x34, 0x67, 0xF4, 0x00, 0x0F, 0xE4, 0xAB, 0x0D, 0xDC,
            0x9A, 0xED, 0xBE, 0x68, 0x47, 0xF9, 0xE0, 0x36, 0x5D, 0xA4, 0x69, 0xE2, 0x24, 0xD8,
            0x5A, 0x85, 0x84, 0xA5, 0x3A, 0xAB, 0xF1, 0x2A, 0x5C, 0x21, 0xBE, 0x3F, 0x4B, 0x00,
            0x0B, 0x00, 0x00, 0xFE, 0x8A, 0x5A, 0xB8, 0x23, 0x9C, 0xB8, 0x03, 0x2B, 0x9A, 0xC0,
            0x3B, 0x43, 0x66, 0xD0, 0x17, 0x86, 0x01, 0x95, 0xB7, 0xD1, 0x49, 0xD0, 0x00, 0x4B,
            0x00, 0x00, 0x00, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xB1, 0x05, 0x00,
            0x19, 0x13, 0xF8, 0x00, 0x18, 0x56, 0x07, 0x02, 0xA4, 0x0A, 0xE7, 0x34, 0x01, 0x0A,
            0xA4, 0xBB, 0x9B, 0x0F, 0xCD, 0x7F, 0x14, 0xBC, 0x50, 0x67, 0xCA, 0x67, 0x81, 0xAB,
            0x74, 0x30, 0x15, 0xE3, 0x7C, 0x71, 0x20, 0xA7, 0x68, 0x4F, 0x54, 0x50, 0x99, 0xD2,
            0xA6, 0x64, 0x00, 0x0C, 0x00, 0x00, 0xFE, 0x8A, 0x4A, 0x31, 0xA1, 0xB5, 0xB8, 0x3C,
            0x86, 0xD6, 0xB4, 0x00, 0x10, 0x34, 0x2D, 0x04, 0x63, 0x7F, 0x0E, 0x3D, 0x6D, 0x85,
            0xBA, 0xC0, 0x47, 0x00, 0x00, 0x00, 0x19, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x31, 0x06, 0x00, 0x2B, 0x13, 0xF8, 0x14, 0x60, 0x00, 0x79, 0x0E, 0x9F, 0xD0, 0x00,
            0x07, 0xBD, 0xEF, 0x7B, 0xDE, 0xF7, 0xBF, 0x4E, 0x29, 0xF6, 0x5C, 0xA1, 0xFA, 0x8C,
            0xB2, 0x37, 0xD1, 0xD0, 0xDB, 0x9E, 0x8A, 0x3A, 0xB8, 0x50, 0x82, 0xAE, 0x7F, 0x40,
            0x94, 0x85, 0x8A, 0xE7, 0x5E, 0x8B, 0x5B, 0x96, 0x89, 0x00, 0x09, 0x00, 0x00, 0xFE,
            0x8A, 0x52, 0x52, 0x89, 0xE4, 0x40, 0x9D, 0x40, 0x01, 0x91, 0x2B, 0x61, 0xE8, 0xAC,
            0x00, 0x4D, 0x00, 0x00, 0x00, 0x1A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xB1,
            0x05, 0x00, 0x3F, 0x13, 0xF8, 0x00, 0x18, 0x00, 0x29, 0xE3, 0x77, 0xF4, 0x00, 0x0F,
            0xCA, 0xB2, 0x2F, 0x30, 0xB6, 0xFF, 0x2A, 0xC4, 0xAD, 0xC3, 0x21, 0xF7, 0x9D, 0xC3,
            0x98, 0xBF, 0xAE, 0xEC, 0xA2, 0xBE, 0x94, 0xAA, 0x86, 0x66, 0xC2, 0xFA, 0xFD, 0x96,
            0x1C, 0x44, 0x5F, 0x2B, 0x80, 0x0B, 0x00, 0x00, 0xFE, 0x8A, 0x56, 0x87, 0x37, 0x56,
            0xB7, 0xBB, 0x62, 0x04, 0x30, 0x3B, 0x91, 0x08, 0x68, 0xE6, 0x64, 0x06, 0xF9, 0x72,
            0xC1, 0x3B, 0x0D, 0x25, 0x72, 0xA0, 0x51, 0x00, 0x00, 0x00, 0x1B, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0xD1, 0x05, 0x00, 0x29, 0x13, 0xF8, 0x00, 0x18, 0x00, 0x27,
            0x73, 0xA7, 0xF4, 0x00, 0x0C, 0x30, 0xC3, 0x0C, 0x30, 0xC3, 0x0C, 0x30, 0xD3, 0x26,
            0x20, 0x6D, 0xF9, 0x61, 0x5D, 0x1C, 0x03, 0x95, 0x51, 0xEE, 0x51, 0x6A, 0x4C, 0x1E,
            0x24, 0xF5, 0x42, 0x3A, 0x7F, 0xFE, 0x36, 0x46, 0x71, 0xC3, 0x10, 0x0E, 0x00, 0x00,
            0xFE, 0x8A, 0x59, 0xD1, 0x1F, 0xF8, 0xD6, 0xDE, 0xD0, 0x66, 0xA7, 0x47, 0x2A, 0x00,
            0x34, 0x35, 0x7F, 0xA3, 0x78, 0x20, 0x2B, 0x64, 0xFB, 0xB7, 0x95, 0xCC, 0x8B, 0xE8,
            0x40, 0x5B, 0x00, 0x00, 0x00, 0x1C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x51,
            0x06, 0x00, 0x3D, 0x13, 0xF8, 0x00, 0x18, 0x43, 0x70, 0x05, 0xCB, 0x85, 0x5E, 0x50,
            0x02, 0xC7, 0x1C, 0x71, 0xC7, 0x1C, 0x71, 0xC6, 0xFE, 0xB3, 0xD5, 0x73, 0x8E, 0xC1,
            0x76, 0xD6, 0x60, 0xAD, 0x50, 0x60, 0xBC, 0xB5, 0x76, 0x2C, 0x25, 0x64, 0x9B, 0xA4,
            0xDB, 0xC0, 0x10, 0x3A, 0x44, 0x13, 0x79, 0x47, 0x89, 0xE8, 0x10, 0x00, 0x00, 0xFE,
            0x8A, 0xE9, 0xB8, 0x95, 0xED, 0x5C, 0xB6, 0x6B, 0x0C, 0x01, 0x58, 0xE4, 0xA9, 0xD4,
            0x00, 0xA1, 0x27, 0x30, 0x2F, 0x70, 0xDF, 0x8E, 0xB7, 0xEC, 0x61, 0x42, 0x52, 0x47,
            0xA6, 0x29, 0xEC, 0xE7, 0xE0, 0x00,
        ];

        let cursor = Cursor::new(data);
        let ivf = Ivf::new(cursor).unwrap();

        assert_eq!(ivf.width(), 176);
        assert_eq!(ivf.height(), 144);
        assert_eq!(ivf.frame_rate_rate(), 30000);
        assert_eq!(ivf.frame_rate_scale(), 1000);
        assert_eq!(ivf.frame_count(), 29);

        let mut first = true;
        let count: usize = ivf
            .iter()
            .map(|frame| {
                if first {
                    assert_eq!(frame.timestamp, 0);
                    first = false;
                } else {
                    assert_ne!(frame.timestamp, 0);
                }

                assert_ne!(frame.data.len(), 0);
                1
            })
            .sum();
        assert_eq!(count, ivf.frame_count() as usize);
    }
}
