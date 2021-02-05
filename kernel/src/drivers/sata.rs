#![allow(dead_code)]

struct FrameInformation {}

impl FrameInformation {
    // Register FIS - host to device
    const TYPE_REG_H2D: u16 = 0x27;
    // Register FIS - device to host
    const TYPE_REG_D2H: u16 = 0x34;
    // DMA activate FIS - device to host
    const TYPE_DMA_ACT: u16 = 0x39;
    // DMA setup FIS - bidirectional
    const TYPE_DMA_SETUP: u16 = 0x41;
    // Data FIS - bidirectional
    const TYPE_DATA: u16 = 0x46;
    // BIST activate FIS - bidirectional
    const TYPE_BIST: u16 = 0x58;
    // PIO setup FIS - device to host
    const TYPE_PIO_SETUP: u16 = 0x5F;
    // Set device bits FIS - device to host
    const TYPE_DEV_BITS: u16 = 0xA1;
}
