use std::cmp::{self, Ordering};

pub fn hexdump(buffer: &[u8], start: u16, end: u16) -> String {
    let mut str = String::new();
    let mut addr = start;
    while addr < end {
        let mut line = format!("{:04x}: ", addr);
        let mut chars = String::new();
        for _ in 0..16 {
            if addr <= end {
                let byte = buffer[addr as usize];
                line.push_str(&format!("{:02x} ", byte));
                let c = byte as char;
                chars.push(if c.is_ascii_graphic() || c == ' ' {
                    c
                } else {
                    '.'
                });

                addr = addr.wrapping_add(1);
            }
        }

        let dump_line = format!("{:>54} {}\n", line, chars);
        str.push_str(&dump_line);

        if addr == 0 {
            break;
        }
    }

    str
}

pub fn compare_slices(a: &[u8], b: &[u8]) -> cmp::Ordering {
    for (ai, bi) in a.iter().zip(b.iter()) {
        match ai.cmp(bi) {
            Ordering::Equal => continue,
            ord => return ord,
        }
    }

    /* if every single element was equal, compare length */
    a.len().cmp(&b.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_slices() {
        let a = [1, 2, 3, 4, 5];
        let b = [1, 2, 3, 4, 5];
        let c = [1, 2, 3, 4, 6];

        println!("{:?}", compare_slices(&a, &b));
        println!("{:?}", compare_slices(&a, &c));
    }
}
