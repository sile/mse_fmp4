use crate::Result;
use std::io::Write;

/// MP4 (ISO BMFF) box.
pub trait Mp4Box {
    /// Box type.
    const BOX_TYPE: [u8; 4];

    /// Box size.
    fn box_size(&self) -> Result<u32> {
        let mut size = 8;
        if self.box_version().is_some() | self.box_flags().is_some() {
            size += 4;
        }
        size += track!(self.box_payload_size())?;
        Ok(size)
    }

    /// Payload size of the box.
    fn box_payload_size(&self) -> Result<u32>;

    /// Box version.
    ///
    /// If this method returns `Some(...)`, the box will be regarded as a full box.
    fn box_version(&self) -> Option<u8> {
        None
    }

    /// Box flags (for full box).
    ///
    /// If this method returns `Some(...)`, the box will be regarded as a full box.
    fn box_flags(&self) -> Option<u32> {
        None
    }

    /// Writes the box to the given writer.
    fn write_box<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, track!(self.box_size())?);
        write_all!(writer, &Self::BOX_TYPE);

        let version = self.box_version();
        let flags = self.box_flags();
        if version.is_some() || flags.is_some() {
            let full_box_header = (u32::from(version.unwrap_or(0)) << 24) | flags.unwrap_or(0);
            write_u32!(writer, full_box_header);
        }

        track!(self.write_box_payload(writer))?;
        Ok(())
    }

    /// Writes the payload of the box to the given writer.
    fn write_box_payload<W: Write>(&self, writer: W) -> Result<()>;
}
