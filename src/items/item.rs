use super::input::Input;
use super::quoted::Quoted;
use super::encoded_word::EncodedWord;


//FEATURE_TODO(non_utf8_input):
// NonUtf8Input(...)

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum Item {
    /// A Item::Input, differs to Input as there can already be some restrictions on it,
    /// e.g. a Item::Input in a Word is meant to be _one_ (possible encoded) word
    Input( Input ),

    /// A Item which is an encoded word
    EncodedWord( EncodedWord ),

    /// A quoted string
    QuotedString( Quoted ),

}
