use {check_len, Error, Result, TryRead, TryWrite};

impl<'a> TryRead<'a> for bool {
    #[inline]
    fn try_read(bytes: &'a [u8], _ctx: ()) -> Result<(Self, usize)> {
        check_len(bytes, 1)?;

        match bytes[0] {
            1 => Ok((true, 1)),
            0 => Ok((false, 1)),
            _ => Err(Error::BadInput {
                err: "Invalid bool encoding",
            }),
        }
    }
}

impl TryWrite for bool {
    #[inline]
    fn try_write(self, bytes: &mut [u8], _ctx: ()) -> Result<usize> {
        check_len(bytes, 1)?;

        bytes[0] = if self { 1 } else { 0 };
        Ok(1)
    }
}
