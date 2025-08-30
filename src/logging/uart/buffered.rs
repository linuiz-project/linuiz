const UART_FIFO_SIZE: usize = 16;
type Chunk = [u8; UART_FIFO_SIZE];
type Chunks = heapless::mpmc::Queue<Chunk, 1024>;

static CHUNKS: Chunks = Chunks::new();


pub unsafe fn send_next_chunk() {
    if let Some(bytes) = CHUNKS.dequeue() {
        let mut uart = UART.get().unwrap().lock();

        #[cfg(debug_assertions)]
        {
            let line_status = uart.read_line_status();
            assert!(line_status.contains(uart::LineStatus::THR_SHR_EMPTY));
        }

        bytes
            .into_iter()
            .take_while(|byte| *byte > 0)
            .for_each(|byte| {
                uart.write_byte(byte);
            });
    }
}
