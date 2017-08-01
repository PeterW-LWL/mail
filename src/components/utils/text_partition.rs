use error:*;
use char_validators::{ is_vchar, is_ws, MailType };

#[derive(Copy, Clone)]
pub enum Partition<'a> {
    //from -> to the start of the next block
    SPACE(&str),
    VCHAR(&str)
}

#[derive(Copy)]
#[repr(bool)]
enum Type { SPACE, VCHAR }

pub fn partition<'a>( text: &'a str ) -> Result< Vec< Partition<'a> > > {
    use self::Type::*;

    if text.len() == 0 {
        return Vec::new()
    }

    // unwrap is ok, as we return earlier if len == 0
    let start_with_vchar = is_vchar( text.chars().next().unwrap(), MailType::Internationalized);

    let mut partitions =  Vec::new();
    let mut current_type = if start_with_vchar { VCHAR } else { SPACE };

    let mut start_of_current = 0;
    for (idx, char) in text.char_indices() {
        if is_vchar( char, MailType::Internationalized ) {
            if current_type == SPACE {
                // idx is the start index of the current char, with is the
                // (exclusive) end index of the previous char which is the
                // last char of the Partition we want to push
                partitions.push( Partition::SPACE( &text[start_of_current..idx] ) );
                start_of_current = idx;
                current_type = VCHAR
            }
        } else if is_ws( char ) || char == '\r' || char == '\n' {
            if current_type == VCHAR {
                partitions.push( Partition::VCHAR( &test[start_of_current..idx] ) );
                start_of_current = idx;
                current_type = SPACE
            }
        } else {
            bail!( "non encodable character found: {:?}", char );
        }
    }


    partitions.push( match current_type {
        SPACE => Partition::SPACE( &test[start_of_current..] ),
        VCHAR => Partition::VCHAR( &test[start_of_current..] )
    } );

    partitions
}