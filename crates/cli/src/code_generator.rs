// pub struct CodeGenerator<'a, 'b> {
//     context: &'a mut Context<'b>,
//     package: String,
//     type_path: Vec<String>,
//     source_info: Option<SourceCodeInfo>,
//     syntax: Syntax,
//     depth: u8,
//     path: Vec<i32>,
//     buf: &'a mut String,
// }
//
// #[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
// #[derive(Clone, PartialEq, ::prost::Message)]
// pub struct FileDescriptorProto {
//     /// file name, relative to root of source tree
//     #[prost(string, optional, tag = "1")]
//     pub name: ::core::option::Option<::prost::alloc::string::String>,
//     /// e.g. "foo", "foo.bar", etc.
//     #[prost(string, optional, tag = "2")]
//     pub package: ::core::option::Option<::prost::alloc::string::String>,
//     /// Names of files imported by this file.
//     #[prost(string, repeated, tag = "3")]
//     pub dependency: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
//     /// Indexes of the public imported files in the dependency list above.
//     #[prost(int32, repeated, packed = "false", tag = "10")]
//     pub public_dependency: ::prost::alloc::vec::Vec<i32>,
//     /// Indexes of the weak imported files in the dependency list.
//     /// For Google-internal migration only. Do not use.
//     #[prost(int32, repeated, packed = "false", tag = "11")]
//     pub weak_dependency: ::prost::alloc::vec::Vec<i32>,
//     /// All top-level definitions in this file.
//     #[prost(message, repeated, tag = "4")]
//     pub message_type: ::prost::alloc::vec::Vec<DescriptorProto>,
//     #[prost(message, repeated, tag = "5")]
//     pub enum_type: ::prost::alloc::vec::Vec<EnumDescriptorProto>,
//     #[prost(message, repeated, tag = "6")]
//     pub service: ::prost::alloc::vec::Vec<ServiceDescriptorProto>,
//     #[prost(message, repeated, tag = "7")]
//     pub extension: ::prost::alloc::vec::Vec<FieldDescriptorProto>,
//     #[prost(message, optional, tag = "8")]
//     pub options: ::core::option::Option<FileOptions>,
//     /// This field contains optional information about the original source code.
//     /// You may safely remove this entire field without harming runtime
//     /// functionality of the descriptors -- the information is needed only by
//     /// development tools.
//     #[prost(message, optional, tag = "9")]
//     pub source_code_info: ::core::option::Option<SourceCodeInfo>,
//     /// The syntax of the proto file.
//     /// The supported values are "proto2" and "proto3".
//     #[prost(string, optional, tag = "12")]
//     pub syntax: ::core::option::Option<::prost::alloc::string::String>,
// }
//
// impl<'b> CodeGenerator<'_, 'b> {
//     fn config(&self) -> &Config {
//         self.context.config()
//     }
//
//     pub(crate) fn generate(context: &mut Context<'b>, file: FileDescriptorProto, buf: &mut String) {}
// }
