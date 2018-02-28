use std::io::Write;

use Result;
use io::ByteCounter;

pub trait Mp4Box {
    const BOX_TYPE: [u8; 4];

    fn box_size(&self) -> Result<u32> {
        let mut size = 8;
        if self.box_version().is_some() | self.box_flags().is_some() {
            size += 4;
        }

        let mut writer = ByteCounter::with_sink();
        track!(self.write_box_payload(&mut writer))?;
        size += writer.count() as u32;

        Ok(size)
    }
    fn box_version(&self) -> Option<u8> {
        None
    }
    fn box_flags(&self) -> Option<u32> {
        None
    }
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
    fn write_box_payload<W: Write>(&self, writer: W) -> Result<()>;
}
