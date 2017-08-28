use std::path::{ Path, PathBuf };
use std::fs::File;
use std::io::{ Write, BufWriter, BufRead, BufReader, Error as IoError };
use std::env;
use std::env::VarError;

fn main() {
    generate_html_header( "./src/headers/headers.gen.spec" ).unwrap();
}


fn generate_html_header<P: AsRef<Path>>( spec: P ) -> Result<(), Error> {
    let out = PathBuf::from( env::var( "OUT_DIR" )? );
    let file = File::open( spec )?;
    let mut enum_output = BufWriter::new( File::create( out.join( "header_enum.rs.partial" ) )? );
    let mut mail_encodable_impl = BufWriter::new( File::create( out.join( "mail_encodable_impl.rs.partial" ) )? );
    let mut decode_match_output = BufWriter::new( File::create( out.join( "decoder_match_cases.rs.partial" ) )? );
    let mut names_output = BufWriter::new( File::create( out.join( "header_enum_names.rs.partial" ) )? );

    writeln!( &mut enum_output, "pub enum Header {{" )?;
    writeln!( &mut mail_encodable_impl, concat!(
        "impl<E> MailEncodable<E> for Header where E: MailEncoder {{\n",
        "    fn encode( &self, encoder:  &mut E ) -> Result<()> {{\n",
        "        use self::Header::*;\n",
        "        match *self {{\n"
    ))?;

    writeln!( &mut decode_match_output, "{{ fn fn_impl(header_name: &str, data: &str) -> Result<Header> {{ match header_name {{" )?;
    writeln!( &mut names_output,
              "{{ fn fn_impl<'a>(header: &'a Header) -> HeaderNameRef<'a> {{ match *header {{" )?;

    let mut next_is_header = true;
    for line in BufReader::new( file ).lines() {
        let line = line?;
        let line = line.trim();
        println!( "LINE: {}", &line );
        if line.starts_with( "--" ) || line.len() == 0 {
            continue;
        }
        let mut parts = line.splitn( 4, "|" ).skip( 1 ).take( 2 );
        let name = parts.next().unwrap().trim();
        let rust_type = parts.next().unwrap().trim();

        if name.len() == 0 && rust_type.len() == 0 {
            continue
        } else if name.len() == 0 {
            panic!( "name missing, but rust type given" );
        } else if rust_type.len() == 0 {
            panic!( "rust type missing, but name given" );
        }

        if next_is_header {
            next_is_header = false;
            assert_eq!( "Name", name );
            assert_eq!( "Rust-Type", rust_type );
            continue;
        }

        assert!( is_valid_header_name( name ) );

        let enum_name = name.replace( "-", "" );

        writeln!( &mut enum_output,
                  "\t{}( {} ),",
                  enum_name, rust_type )?;

        writeln!( &mut names_output,
                  "\t{}( .. ) => unsafe {{ HeaderNameRef::Static( AsciiStr::from_ascii_unchecked( {:?} ) ) }},",
                  enum_name, name )?;

        writeln!( &mut mail_encodable_impl,
                  concat!( "        {}( ref field ) => encode_header_helper(",
                           " unsafe {{ AsciiStr::from_ascii_unchecked( {:?} ) }},",
                           " field,",
                           " encoder)," ),
                  enum_name, name )?;

        writeln!( &mut decode_match_output,
                  r"\t{:?} => Self::{}( {}::decode( data )? ),",
                  name, enum_name, rust_type )?;

    }

    writeln!( &mut enum_output,
              "\tOther( HeaderName, Unstructured )" )?;
    writeln!( &mut enum_output, "}}" )?;

    writeln!( &mut names_output,
              "Other( ref name, .. ) => HeaderNameRef::Other( &*name )" )?;
    writeln!( &mut names_output, "}} }} fn_impl }}")?;

    writeln!( &mut mail_encodable_impl,
              "\tOther( ref name, ref value ) => encode_header_helper( &*name, value, encoder )")?;
    writeln!( &mut mail_encodable_impl, "}} }} }}")?;

    writeln!( &mut decode_match_output,
              r"\tname => Self::Other( HeaderName::new( name )?, Unstructured::decode( data )? )" )?;
    writeln!( &mut decode_match_output, "}} }} fn_impl }}")?;

    Ok( () )
}


#[derive(Debug)]
enum Error {
    IoError(IoError),
    VarError(VarError)
}

impl From<IoError> for Error {
    fn from( err: IoError ) -> Error {
        Error::IoError( err )
    }
}

impl From<VarError> for Error {
    fn from( err: VarError ) -> Error {
        Error::VarError( err )
    }
}

fn is_valid_header_name( name: &str ) -> bool {
    name.as_bytes().iter().all( |b| {
        match *b {
            b'a'...b'z' |
            b'A'...b'Z' |
            b'0'...b'9' |
            b'-' => true,
            _ => false
        }
    })
}
