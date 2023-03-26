use bytes::{Bytes};

#[derive(Debug, Clone)]
pub struct Message {
    pub size: u64, //Size of the message (total including size of size and data_type)
    pub hit_size: u64,
    pub data_type: u16,
    pub data: Vec<u8>
}

impl Default for Message {
    fn default() -> Message {
        Message { size: 64 * 2 + 16, hit_size: 0, data_type: 0, data: vec![] }
    }
}

pub fn convert_messages_to_bytes(mess_list: Vec<Message>) -> Bytes {

    let mut binary: Vec<u8> = Vec::new();
    let mut total_data: usize = 0;
    mess_list.iter().for_each(|mess| {
        total_data += mess.size as usize
    });
    binary.reserve(total_data);
    for mut mess in mess_list {
        binary.append(&mut mess.size.to_ne_bytes().to_vec());
        binary.append(&mut mess.hit_size.to_ne_bytes().to_vec());
        binary.append(&mut mess.data_type.to_ne_bytes().to_vec());
        binary.append(&mut mess.data);
    }

    Bytes::from(binary)
}