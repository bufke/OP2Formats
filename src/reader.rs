//! Little-endian cursor over a byte slice, with errors that say where parsing stopped.

use crate::Error;

pub(crate) struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn at(data: &'a [u8], pos: usize) -> Self {
        Self { data, pos }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn bytes(&mut self, len: usize) -> Result<&'a [u8], Error> {
        let end = self.pos.checked_add(len).filter(|&end| end <= self.data.len()).ok_or(Error::Truncated {
            offset: self.pos,
            wanted: len,
        })?;
        let slice = &self.data[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    pub fn skip(&mut self, len: usize) -> Result<(), Error> {
        self.bytes(len).map(|_| ())
    }

    pub fn u16(&mut self) -> Result<u16, Error> {
        Ok(u16::from_le_bytes(self.bytes(2)?.try_into().unwrap()))
    }

    pub fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }

    pub fn i32(&mut self) -> Result<i32, Error> {
        Ok(i32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }

    pub fn tag(&mut self) -> Result<[u8; 4], Error> {
        Ok(self.bytes(4)?.try_into().unwrap())
    }

    /// Reads a 4-byte tag and fails unless it matches `expected`.
    pub fn expect_tag(&mut self, expected: &[u8; 4]) -> Result<(), Error> {
        let offset = self.pos;
        let found = self.tag()?;
        if &found != expected {
            return Err(Error::BadTag { offset, expected: *expected, found });
        }
        Ok(())
    }

    /// String prefixed by a u32 byte length (not NUL-terminated).
    pub fn len_string(&mut self) -> Result<String, Error> {
        let len = self.u32()? as usize;
        Ok(String::from_utf8_lossy(self.bytes(len)?).into_owned())
    }
}
