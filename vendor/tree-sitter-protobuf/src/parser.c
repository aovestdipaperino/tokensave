#include <tree_sitter/parser.h>

#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wmissing-field-initializers"
#endif

#ifdef _MSC_VER
#pragma optimize("", off)
#elif defined(__clang__)
#pragma clang optimize off
#elif defined(__GNUC__)
#pragma GCC optimize ("O0")
#endif

#define LANGUAGE_VERSION 13
#define STATE_COUNT 252
#define LARGE_STATE_COUNT 2
#define SYMBOL_COUNT 115
#define ALIAS_COUNT 0
#define TOKEN_COUNT 61
#define EXTERNAL_TOKEN_COUNT 0
#define FIELD_COUNT 0
#define MAX_ALIAS_SEQUENCE_LENGTH 14
#define PRODUCTION_ID_COUNT 1

enum {
  sym_comment = 1,
  anon_sym_syntax = 2,
  anon_sym_EQ = 3,
  aux_sym_syntax_token1 = 4,
  anon_sym_SEMI = 5,
  anon_sym_package = 6,
  anon_sym_import = 7,
  anon_sym_option = 8,
  anon_sym_LPAREN = 9,
  anon_sym_RPAREN = 10,
  anon_sym_enum = 11,
  anon_sym_LBRACE = 12,
  anon_sym_RBRACE = 13,
  anon_sym_DASH = 14,
  anon_sym_LBRACK = 15,
  anon_sym_COMMA = 16,
  anon_sym_RBRACK = 17,
  anon_sym_message = 18,
  anon_sym_service = 19,
  anon_sym_rpc = 20,
  anon_sym_stream = 21,
  anon_sym_returns = 22,
  anon_sym_repeated = 23,
  anon_sym_oneof = 24,
  anon_sym_map = 25,
  anon_sym_LT = 26,
  anon_sym_GT = 27,
  anon_sym_int32 = 28,
  anon_sym_int64 = 29,
  anon_sym_uint32 = 30,
  anon_sym_uint64 = 31,
  anon_sym_sint32 = 32,
  anon_sym_sint64 = 33,
  anon_sym_fixed32 = 34,
  anon_sym_fixed64 = 35,
  anon_sym_sfixed32 = 36,
  anon_sym_sfixed64 = 37,
  anon_sym_bool = 38,
  anon_sym_string = 39,
  anon_sym_reserved = 40,
  anon_sym_to = 41,
  anon_sym_max = 42,
  sym_intLit = 43,
  aux_sym_floatLit_token1 = 44,
  aux_sym_floatLit_token2 = 45,
  aux_sym_floatLit_token3 = 46,
  anon_sym_inf = 47,
  anon_sym_nan = 48,
  sym_boolLit = 49,
  anon_sym_DQUOTE = 50,
  aux_sym_strLit_token1 = 51,
  anon_sym_SQUOTE = 52,
  aux_sym_strLit_token2 = 53,
  anon_sym_double = 54,
  anon_sym_float = 55,
  anon_sym_bytes = 56,
  aux_sym_constant_token1 = 57,
  anon_sym_COLON = 58,
  sym_ident = 59,
  anon_sym_DOT = 60,
  sym_source_file = 61,
  sym_syntax = 62,
  sym_package = 63,
  sym_import = 64,
  sym_option = 65,
  sym_optionName = 66,
  sym_enum = 67,
  sym_enumBody = 68,
  sym_enumField = 69,
  sym_enumValueOption = 70,
  sym_message = 71,
  sym_messageBody = 72,
  sym_service = 73,
  sym_rpc = 74,
  sym_field = 75,
  sym_fieldOptions = 76,
  sym_fieldOption = 77,
  sym_oneof = 78,
  sym_oneofField = 79,
  sym_mapField = 80,
  sym_keyType = 81,
  sym_reserved = 82,
  sym_ranges = 83,
  sym_range = 84,
  sym_fieldNames = 85,
  sym_floatLit = 86,
  sym_strLit = 87,
  sym_type = 88,
  sym_fieldNumber = 89,
  sym_emptyStatement = 90,
  sym_constant = 91,
  sym_msgLit = 92,
  sym_fullIdent = 93,
  sym_messageName = 94,
  sym_mapName = 95,
  sym_enumName = 96,
  sym_fieldName = 97,
  sym_oneofName = 98,
  sym_serviceName = 99,
  sym_rpcName = 100,
  sym_enumMessageType = 101,
  aux_sym_source_file_repeat1 = 102,
  aux_sym_enumBody_repeat1 = 103,
  aux_sym_enumField_repeat1 = 104,
  aux_sym_messageBody_repeat1 = 105,
  aux_sym_service_repeat1 = 106,
  aux_sym_rpc_repeat1 = 107,
  aux_sym_fieldOptions_repeat1 = 108,
  aux_sym_oneof_repeat1 = 109,
  aux_sym_ranges_repeat1 = 110,
  aux_sym_fieldNames_repeat1 = 111,
  aux_sym_msgLit_repeat1 = 112,
  aux_sym_fullIdent_repeat1 = 113,
  aux_sym_enumMessageType_repeat1 = 114,
};

static const char * const ts_symbol_names[] = {
  [ts_builtin_sym_end] = "end",
  [sym_comment] = "comment",
  [anon_sym_syntax] = "syntax",
  [anon_sym_EQ] = "=",
  [aux_sym_syntax_token1] = "syntax_token1",
  [anon_sym_SEMI] = ";",
  [anon_sym_package] = "package",
  [anon_sym_import] = "import",
  [anon_sym_option] = "option",
  [anon_sym_LPAREN] = "(",
  [anon_sym_RPAREN] = ")",
  [anon_sym_enum] = "enum",
  [anon_sym_LBRACE] = "{",
  [anon_sym_RBRACE] = "}",
  [anon_sym_DASH] = "-",
  [anon_sym_LBRACK] = "[",
  [anon_sym_COMMA] = ",",
  [anon_sym_RBRACK] = "]",
  [anon_sym_message] = "message",
  [anon_sym_service] = "service",
  [anon_sym_rpc] = "rpc",
  [anon_sym_stream] = "stream",
  [anon_sym_returns] = "returns",
  [anon_sym_repeated] = "repeated",
  [anon_sym_oneof] = "oneof",
  [anon_sym_map] = "map",
  [anon_sym_LT] = "<",
  [anon_sym_GT] = ">",
  [anon_sym_int32] = "int32",
  [anon_sym_int64] = "int64",
  [anon_sym_uint32] = "uint32",
  [anon_sym_uint64] = "uint64",
  [anon_sym_sint32] = "sint32",
  [anon_sym_sint64] = "sint64",
  [anon_sym_fixed32] = "fixed32",
  [anon_sym_fixed64] = "fixed64",
  [anon_sym_sfixed32] = "sfixed32",
  [anon_sym_sfixed64] = "sfixed64",
  [anon_sym_bool] = "bool",
  [anon_sym_string] = "string",
  [anon_sym_reserved] = "reserved",
  [anon_sym_to] = "to",
  [anon_sym_max] = "max",
  [sym_intLit] = "intLit",
  [aux_sym_floatLit_token1] = "floatLit_token1",
  [aux_sym_floatLit_token2] = "floatLit_token2",
  [aux_sym_floatLit_token3] = "floatLit_token3",
  [anon_sym_inf] = "inf",
  [anon_sym_nan] = "nan",
  [sym_boolLit] = "boolLit",
  [anon_sym_DQUOTE] = "\"",
  [aux_sym_strLit_token1] = "strLit_token1",
  [anon_sym_SQUOTE] = "'",
  [aux_sym_strLit_token2] = "strLit_token2",
  [anon_sym_double] = "double",
  [anon_sym_float] = "float",
  [anon_sym_bytes] = "bytes",
  [aux_sym_constant_token1] = "constant_token1",
  [anon_sym_COLON] = ":",
  [sym_ident] = "ident",
  [anon_sym_DOT] = ".",
  [sym_source_file] = "source_file",
  [sym_syntax] = "syntax",
  [sym_package] = "package",
  [sym_import] = "import",
  [sym_option] = "option",
  [sym_optionName] = "optionName",
  [sym_enum] = "enum",
  [sym_enumBody] = "enumBody",
  [sym_enumField] = "enumField",
  [sym_enumValueOption] = "enumValueOption",
  [sym_message] = "message",
  [sym_messageBody] = "messageBody",
  [sym_service] = "service",
  [sym_rpc] = "rpc",
  [sym_field] = "field",
  [sym_fieldOptions] = "fieldOptions",
  [sym_fieldOption] = "fieldOption",
  [sym_oneof] = "oneof",
  [sym_oneofField] = "oneofField",
  [sym_mapField] = "mapField",
  [sym_keyType] = "keyType",
  [sym_reserved] = "reserved",
  [sym_ranges] = "ranges",
  [sym_range] = "range",
  [sym_fieldNames] = "fieldNames",
  [sym_floatLit] = "floatLit",
  [sym_strLit] = "strLit",
  [sym_type] = "type",
  [sym_fieldNumber] = "fieldNumber",
  [sym_emptyStatement] = "emptyStatement",
  [sym_constant] = "constant",
  [sym_msgLit] = "msgLit",
  [sym_fullIdent] = "fullIdent",
  [sym_messageName] = "messageName",
  [sym_mapName] = "mapName",
  [sym_enumName] = "enumName",
  [sym_fieldName] = "fieldName",
  [sym_oneofName] = "oneofName",
  [sym_serviceName] = "serviceName",
  [sym_rpcName] = "rpcName",
  [sym_enumMessageType] = "enumMessageType",
  [aux_sym_source_file_repeat1] = "source_file_repeat1",
  [aux_sym_enumBody_repeat1] = "enumBody_repeat1",
  [aux_sym_enumField_repeat1] = "enumField_repeat1",
  [aux_sym_messageBody_repeat1] = "messageBody_repeat1",
  [aux_sym_service_repeat1] = "service_repeat1",
  [aux_sym_rpc_repeat1] = "rpc_repeat1",
  [aux_sym_fieldOptions_repeat1] = "fieldOptions_repeat1",
  [aux_sym_oneof_repeat1] = "oneof_repeat1",
  [aux_sym_ranges_repeat1] = "ranges_repeat1",
  [aux_sym_fieldNames_repeat1] = "fieldNames_repeat1",
  [aux_sym_msgLit_repeat1] = "msgLit_repeat1",
  [aux_sym_fullIdent_repeat1] = "fullIdent_repeat1",
  [aux_sym_enumMessageType_repeat1] = "enumMessageType_repeat1",
};

static const TSSymbol ts_symbol_map[] = {
  [ts_builtin_sym_end] = ts_builtin_sym_end,
  [sym_comment] = sym_comment,
  [anon_sym_syntax] = anon_sym_syntax,
  [anon_sym_EQ] = anon_sym_EQ,
  [aux_sym_syntax_token1] = aux_sym_syntax_token1,
  [anon_sym_SEMI] = anon_sym_SEMI,
  [anon_sym_package] = anon_sym_package,
  [anon_sym_import] = anon_sym_import,
  [anon_sym_option] = anon_sym_option,
  [anon_sym_LPAREN] = anon_sym_LPAREN,
  [anon_sym_RPAREN] = anon_sym_RPAREN,
  [anon_sym_enum] = anon_sym_enum,
  [anon_sym_LBRACE] = anon_sym_LBRACE,
  [anon_sym_RBRACE] = anon_sym_RBRACE,
  [anon_sym_DASH] = anon_sym_DASH,
  [anon_sym_LBRACK] = anon_sym_LBRACK,
  [anon_sym_COMMA] = anon_sym_COMMA,
  [anon_sym_RBRACK] = anon_sym_RBRACK,
  [anon_sym_message] = anon_sym_message,
  [anon_sym_service] = anon_sym_service,
  [anon_sym_rpc] = anon_sym_rpc,
  [anon_sym_stream] = anon_sym_stream,
  [anon_sym_returns] = anon_sym_returns,
  [anon_sym_repeated] = anon_sym_repeated,
  [anon_sym_oneof] = anon_sym_oneof,
  [anon_sym_map] = anon_sym_map,
  [anon_sym_LT] = anon_sym_LT,
  [anon_sym_GT] = anon_sym_GT,
  [anon_sym_int32] = anon_sym_int32,
  [anon_sym_int64] = anon_sym_int64,
  [anon_sym_uint32] = anon_sym_uint32,
  [anon_sym_uint64] = anon_sym_uint64,
  [anon_sym_sint32] = anon_sym_sint32,
  [anon_sym_sint64] = anon_sym_sint64,
  [anon_sym_fixed32] = anon_sym_fixed32,
  [anon_sym_fixed64] = anon_sym_fixed64,
  [anon_sym_sfixed32] = anon_sym_sfixed32,
  [anon_sym_sfixed64] = anon_sym_sfixed64,
  [anon_sym_bool] = anon_sym_bool,
  [anon_sym_string] = anon_sym_string,
  [anon_sym_reserved] = anon_sym_reserved,
  [anon_sym_to] = anon_sym_to,
  [anon_sym_max] = anon_sym_max,
  [sym_intLit] = sym_intLit,
  [aux_sym_floatLit_token1] = aux_sym_floatLit_token1,
  [aux_sym_floatLit_token2] = aux_sym_floatLit_token2,
  [aux_sym_floatLit_token3] = aux_sym_floatLit_token3,
  [anon_sym_inf] = anon_sym_inf,
  [anon_sym_nan] = anon_sym_nan,
  [sym_boolLit] = sym_boolLit,
  [anon_sym_DQUOTE] = anon_sym_DQUOTE,
  [aux_sym_strLit_token1] = aux_sym_strLit_token1,
  [anon_sym_SQUOTE] = anon_sym_SQUOTE,
  [aux_sym_strLit_token2] = aux_sym_strLit_token2,
  [anon_sym_double] = anon_sym_double,
  [anon_sym_float] = anon_sym_float,
  [anon_sym_bytes] = anon_sym_bytes,
  [aux_sym_constant_token1] = aux_sym_constant_token1,
  [anon_sym_COLON] = anon_sym_COLON,
  [sym_ident] = sym_ident,
  [anon_sym_DOT] = anon_sym_DOT,
  [sym_source_file] = sym_source_file,
  [sym_syntax] = sym_syntax,
  [sym_package] = sym_package,
  [sym_import] = sym_import,
  [sym_option] = sym_option,
  [sym_optionName] = sym_optionName,
  [sym_enum] = sym_enum,
  [sym_enumBody] = sym_enumBody,
  [sym_enumField] = sym_enumField,
  [sym_enumValueOption] = sym_enumValueOption,
  [sym_message] = sym_message,
  [sym_messageBody] = sym_messageBody,
  [sym_service] = sym_service,
  [sym_rpc] = sym_rpc,
  [sym_field] = sym_field,
  [sym_fieldOptions] = sym_fieldOptions,
  [sym_fieldOption] = sym_fieldOption,
  [sym_oneof] = sym_oneof,
  [sym_oneofField] = sym_oneofField,
  [sym_mapField] = sym_mapField,
  [sym_keyType] = sym_keyType,
  [sym_reserved] = sym_reserved,
  [sym_ranges] = sym_ranges,
  [sym_range] = sym_range,
  [sym_fieldNames] = sym_fieldNames,
  [sym_floatLit] = sym_floatLit,
  [sym_strLit] = sym_strLit,
  [sym_type] = sym_type,
  [sym_fieldNumber] = sym_fieldNumber,
  [sym_emptyStatement] = sym_emptyStatement,
  [sym_constant] = sym_constant,
  [sym_msgLit] = sym_msgLit,
  [sym_fullIdent] = sym_fullIdent,
  [sym_messageName] = sym_messageName,
  [sym_mapName] = sym_mapName,
  [sym_enumName] = sym_enumName,
  [sym_fieldName] = sym_fieldName,
  [sym_oneofName] = sym_oneofName,
  [sym_serviceName] = sym_serviceName,
  [sym_rpcName] = sym_rpcName,
  [sym_enumMessageType] = sym_enumMessageType,
  [aux_sym_source_file_repeat1] = aux_sym_source_file_repeat1,
  [aux_sym_enumBody_repeat1] = aux_sym_enumBody_repeat1,
  [aux_sym_enumField_repeat1] = aux_sym_enumField_repeat1,
  [aux_sym_messageBody_repeat1] = aux_sym_messageBody_repeat1,
  [aux_sym_service_repeat1] = aux_sym_service_repeat1,
  [aux_sym_rpc_repeat1] = aux_sym_rpc_repeat1,
  [aux_sym_fieldOptions_repeat1] = aux_sym_fieldOptions_repeat1,
  [aux_sym_oneof_repeat1] = aux_sym_oneof_repeat1,
  [aux_sym_ranges_repeat1] = aux_sym_ranges_repeat1,
  [aux_sym_fieldNames_repeat1] = aux_sym_fieldNames_repeat1,
  [aux_sym_msgLit_repeat1] = aux_sym_msgLit_repeat1,
  [aux_sym_fullIdent_repeat1] = aux_sym_fullIdent_repeat1,
  [aux_sym_enumMessageType_repeat1] = aux_sym_enumMessageType_repeat1,
};

static const TSSymbolMetadata ts_symbol_metadata[] = {
  [ts_builtin_sym_end] = {
    .visible = false,
    .named = true,
  },
  [sym_comment] = {
    .visible = true,
    .named = true,
  },
  [anon_sym_syntax] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_EQ] = {
    .visible = true,
    .named = false,
  },
  [aux_sym_syntax_token1] = {
    .visible = false,
    .named = false,
  },
  [anon_sym_SEMI] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_package] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_import] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_option] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_LPAREN] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_RPAREN] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_enum] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_LBRACE] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_RBRACE] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_DASH] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_LBRACK] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_COMMA] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_RBRACK] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_message] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_service] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_rpc] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_stream] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_returns] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_repeated] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_oneof] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_map] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_LT] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_GT] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_int32] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_int64] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_uint32] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_uint64] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_sint32] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_sint64] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_fixed32] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_fixed64] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_sfixed32] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_sfixed64] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_bool] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_string] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_reserved] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_to] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_max] = {
    .visible = true,
    .named = false,
  },
  [sym_intLit] = {
    .visible = true,
    .named = true,
  },
  [aux_sym_floatLit_token1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_floatLit_token2] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_floatLit_token3] = {
    .visible = false,
    .named = false,
  },
  [anon_sym_inf] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_nan] = {
    .visible = true,
    .named = false,
  },
  [sym_boolLit] = {
    .visible = true,
    .named = true,
  },
  [anon_sym_DQUOTE] = {
    .visible = true,
    .named = false,
  },
  [aux_sym_strLit_token1] = {
    .visible = false,
    .named = false,
  },
  [anon_sym_SQUOTE] = {
    .visible = true,
    .named = false,
  },
  [aux_sym_strLit_token2] = {
    .visible = false,
    .named = false,
  },
  [anon_sym_double] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_float] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_bytes] = {
    .visible = true,
    .named = false,
  },
  [aux_sym_constant_token1] = {
    .visible = false,
    .named = false,
  },
  [anon_sym_COLON] = {
    .visible = true,
    .named = false,
  },
  [sym_ident] = {
    .visible = true,
    .named = true,
  },
  [anon_sym_DOT] = {
    .visible = true,
    .named = false,
  },
  [sym_source_file] = {
    .visible = true,
    .named = true,
  },
  [sym_syntax] = {
    .visible = true,
    .named = true,
  },
  [sym_package] = {
    .visible = true,
    .named = true,
  },
  [sym_import] = {
    .visible = true,
    .named = true,
  },
  [sym_option] = {
    .visible = true,
    .named = true,
  },
  [sym_optionName] = {
    .visible = true,
    .named = true,
  },
  [sym_enum] = {
    .visible = true,
    .named = true,
  },
  [sym_enumBody] = {
    .visible = true,
    .named = true,
  },
  [sym_enumField] = {
    .visible = true,
    .named = true,
  },
  [sym_enumValueOption] = {
    .visible = true,
    .named = true,
  },
  [sym_message] = {
    .visible = true,
    .named = true,
  },
  [sym_messageBody] = {
    .visible = true,
    .named = true,
  },
  [sym_service] = {
    .visible = true,
    .named = true,
  },
  [sym_rpc] = {
    .visible = true,
    .named = true,
  },
  [sym_field] = {
    .visible = true,
    .named = true,
  },
  [sym_fieldOptions] = {
    .visible = true,
    .named = true,
  },
  [sym_fieldOption] = {
    .visible = true,
    .named = true,
  },
  [sym_oneof] = {
    .visible = true,
    .named = true,
  },
  [sym_oneofField] = {
    .visible = true,
    .named = true,
  },
  [sym_mapField] = {
    .visible = true,
    .named = true,
  },
  [sym_keyType] = {
    .visible = true,
    .named = true,
  },
  [sym_reserved] = {
    .visible = true,
    .named = true,
  },
  [sym_ranges] = {
    .visible = true,
    .named = true,
  },
  [sym_range] = {
    .visible = true,
    .named = true,
  },
  [sym_fieldNames] = {
    .visible = true,
    .named = true,
  },
  [sym_floatLit] = {
    .visible = true,
    .named = true,
  },
  [sym_strLit] = {
    .visible = true,
    .named = true,
  },
  [sym_type] = {
    .visible = true,
    .named = true,
  },
  [sym_fieldNumber] = {
    .visible = true,
    .named = true,
  },
  [sym_emptyStatement] = {
    .visible = true,
    .named = true,
  },
  [sym_constant] = {
    .visible = true,
    .named = true,
  },
  [sym_msgLit] = {
    .visible = true,
    .named = true,
  },
  [sym_fullIdent] = {
    .visible = true,
    .named = true,
  },
  [sym_messageName] = {
    .visible = true,
    .named = true,
  },
  [sym_mapName] = {
    .visible = true,
    .named = true,
  },
  [sym_enumName] = {
    .visible = true,
    .named = true,
  },
  [sym_fieldName] = {
    .visible = true,
    .named = true,
  },
  [sym_oneofName] = {
    .visible = true,
    .named = true,
  },
  [sym_serviceName] = {
    .visible = true,
    .named = true,
  },
  [sym_rpcName] = {
    .visible = true,
    .named = true,
  },
  [sym_enumMessageType] = {
    .visible = true,
    .named = true,
  },
  [aux_sym_source_file_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_enumBody_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_enumField_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_messageBody_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_service_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_rpc_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_fieldOptions_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_oneof_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_ranges_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_fieldNames_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_msgLit_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_fullIdent_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_enumMessageType_repeat1] = {
    .visible = false,
    .named = false,
  },
};

static const TSSymbol ts_alias_sequences[PRODUCTION_ID_COUNT][MAX_ALIAS_SEQUENCE_LENGTH] = {
  [0] = {0},
};

static const uint16_t ts_non_terminal_alias_map[] = {
  0,
};

static inline bool aux_sym_strLit_token2_character_set_1(int32_t c) {
  return (c < 'f'
    ? (c < '\\'
      ? (c < '\''
        ? c == '"'
        : c <= '\'')
      : (c <= '\\' || (c >= 'a' && c <= 'b')))
    : (c <= 'f' || (c < 't'
      ? (c < 'r'
        ? c == 'n'
        : c <= 'r')
      : (c <= 't' || c == 'v'))));
}

static bool ts_lex(TSLexer *lexer, TSStateId state) {
  START_LEXER();
  eof = lexer->eof(lexer);
  switch (state) {
    case 0:
      if (eof) ADVANCE(149);
      if (lookahead == '"') ADVANCE(243);
      if (lookahead == '\'') ADVANCE(247);
      if (lookahead == '(') ADVANCE(171);
      if (lookahead == ')') ADVANCE(172);
      if (lookahead == '+') ADVANCE(257);
      if (lookahead == ',') ADVANCE(179);
      if (lookahead == '-') ADVANCE(177);
      if (lookahead == '.') ADVANCE(357);
      if (lookahead == '/') ADVANCE(12);
      if (lookahead == '0') ADVANCE(225);
      if (lookahead == ':') ADVANCE(258);
      if (lookahead == ';') ADVANCE(166);
      if (lookahead == '<') ADVANCE(194);
      if (lookahead == '=') ADVANCE(164);
      if (lookahead == '>') ADVANCE(195);
      if (lookahead == 'E') ADVANCE(135);
      if (lookahead == '[') ADVANCE(178);
      if (lookahead == ']') ADVANCE(180);
      if (lookahead == 'b') ADVANCE(95);
      if (lookahead == 'd') ADVANCE(91);
      if (lookahead == 'e') ADVANCE(84);
      if (lookahead == 'f') ADVANCE(29);
      if (lookahead == 'i') ADVANCE(78);
      if (lookahead == 'm') ADVANCE(30);
      if (lookahead == 'n') ADVANCE(31);
      if (lookahead == 'o') ADVANCE(81);
      if (lookahead == 'p') ADVANCE(32);
      if (lookahead == 'r') ADVANCE(48);
      if (lookahead == 's') ADVANCE(49);
      if (lookahead == 't') ADVANCE(92);
      if (lookahead == 'u') ADVANCE(73);
      if (lookahead == '{') ADVANCE(175);
      if (lookahead == '}') ADVANCE(176);
      if (lookahead == '\t' ||
          lookahead == '\n' ||
          lookahead == '\r' ||
          lookahead == ' ') SKIP(0)
      if (('1' <= lookahead && lookahead <= '9')) ADVANCE(226);
      END_STATE();
    case 1:
      if (lookahead == '"') ADVANCE(243);
      if (lookahead == '\'') ADVANCE(247);
      if (lookahead == '.') ADVANCE(138);
      if (lookahead == '/') ADVANCE(12);
      if (lookahead == '0') ADVANCE(225);
      if (lookahead == 'f') ADVANCE(277);
      if (lookahead == 'i') ADVANCE(319);
      if (lookahead == 'n') ADVANCE(279);
      if (lookahead == 't') ADVANCE(334);
      if (lookahead == '{') ADVANCE(175);
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(257);
      if (lookahead == 'E' ||
          lookahead == 'e') ADVANCE(355);
      if (lookahead == '\t' ||
          lookahead == '\n' ||
          lookahead == '\r' ||
          lookahead == ' ') SKIP(1)
      if (('1' <= lookahead && lookahead <= '9')) ADVANCE(226);
      if (('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 2:
      if (lookahead == '"') ADVANCE(165);
      END_STATE();
    case 3:
      if (lookahead == '"') ADVANCE(104);
      if (lookahead == ',') ADVANCE(179);
      if (lookahead == '-') ADVANCE(177);
      if (lookahead == '.') ADVANCE(357);
      if (lookahead == '/') ADVANCE(12);
      if (lookahead == '0') ADVANCE(229);
      if (lookahead == ':') ADVANCE(258);
      if (lookahead == ';') ADVANCE(166);
      if (lookahead == '=') ADVANCE(164);
      if (lookahead == 'b') ADVANCE(328);
      if (lookahead == 'd') ADVANCE(323);
      if (lookahead == 'e') ADVANCE(318);
      if (lookahead == 'f') ADVANCE(304);
      if (lookahead == 'i') ADVANCE(317);
      if (lookahead == 'm') ADVANCE(274);
      if (lookahead == 'o') ADVANCE(316);
      if (lookahead == 'r') ADVANCE(286);
      if (lookahead == 's') ADVANCE(301);
      if (lookahead == 'u') ADVANCE(307);
      if (lookahead == '}') ADVANCE(176);
      if (lookahead == '\t' ||
          lookahead == '\n' ||
          lookahead == '\r' ||
          lookahead == ' ') SKIP(3)
      if (('1' <= lookahead && lookahead <= '9')) ADVANCE(231);
      if (('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 4:
      if (lookahead == '(') ADVANCE(171);
      if (lookahead == ')') ADVANCE(172);
      if (lookahead == ',') ADVANCE(179);
      if (lookahead == '.') ADVANCE(357);
      if (lookahead == '/') ADVANCE(12);
      if (lookahead == '0') ADVANCE(229);
      if (lookahead == ';') ADVANCE(166);
      if (lookahead == '=') ADVANCE(164);
      if (lookahead == '>') ADVANCE(195);
      if (lookahead == ']') ADVANCE(180);
      if (lookahead == '}') ADVANCE(176);
      if (lookahead == '\t' ||
          lookahead == '\n' ||
          lookahead == '\r' ||
          lookahead == ' ') SKIP(4)
      if (('1' <= lookahead && lookahead <= '9')) ADVANCE(231);
      if (('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 5:
      if (lookahead == ',') ADVANCE(179);
      if (lookahead == '.') ADVANCE(357);
      if (lookahead == '/') ADVANCE(12);
      if (lookahead == ';') ADVANCE(166);
      if (lookahead == 'b') ADVANCE(328);
      if (lookahead == 'd') ADVANCE(323);
      if (lookahead == 'e') ADVANCE(318);
      if (lookahead == 'f') ADVANCE(304);
      if (lookahead == 'i') ADVANCE(317);
      if (lookahead == 'm') ADVANCE(274);
      if (lookahead == 'o') ADVANCE(316);
      if (lookahead == 'r') ADVANCE(286);
      if (lookahead == 's') ADVANCE(301);
      if (lookahead == 't') ADVANCE(327);
      if (lookahead == 'u') ADVANCE(307);
      if (lookahead == '}') ADVANCE(176);
      if (lookahead == '\t' ||
          lookahead == '\n' ||
          lookahead == '\r' ||
          lookahead == ' ') SKIP(5)
      if (('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 6:
      if (lookahead == '.') ADVANCE(357);
      if (lookahead == '/') ADVANCE(12);
      if (lookahead == ';') ADVANCE(166);
      if (lookahead == 'b') ADVANCE(328);
      if (lookahead == 'd') ADVANCE(323);
      if (lookahead == 'f') ADVANCE(304);
      if (lookahead == 'i') ADVANCE(317);
      if (lookahead == 'o') ADVANCE(331);
      if (lookahead == 's') ADVANCE(301);
      if (lookahead == 'u') ADVANCE(307);
      if (lookahead == '}') ADVANCE(176);
      if (lookahead == '\t' ||
          lookahead == '\n' ||
          lookahead == '\r' ||
          lookahead == ' ') SKIP(6)
      if (('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 7:
      if (lookahead == '.') ADVANCE(357);
      if (lookahead == '/') ADVANCE(12);
      if (lookahead == 'b') ADVANCE(328);
      if (lookahead == 'd') ADVANCE(323);
      if (lookahead == 'f') ADVANCE(304);
      if (lookahead == 'i') ADVANCE(317);
      if (lookahead == 's') ADVANCE(301);
      if (lookahead == 'u') ADVANCE(307);
      if (lookahead == '\t' ||
          lookahead == '\n' ||
          lookahead == '\r' ||
          lookahead == ' ') SKIP(7)
      if (('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 8:
      if (lookahead == '.') ADVANCE(357);
      if (lookahead == '/') ADVANCE(12);
      if (lookahead == 's') ADVANCE(345);
      if (lookahead == '\t' ||
          lookahead == '\n' ||
          lookahead == '\r' ||
          lookahead == ' ') SKIP(8)
      if (('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 9:
      if (lookahead == '.') ADVANCE(138);
      if (lookahead == '/') ADVANCE(12);
      if (lookahead == '0') ADVANCE(225);
      if (lookahead == 'i') ADVANCE(85);
      if (lookahead == 'n') ADVANCE(31);
      if (lookahead == 'E' ||
          lookahead == 'e') ADVANCE(135);
      if (lookahead == '\t' ||
          lookahead == '\n' ||
          lookahead == '\r' ||
          lookahead == ' ') SKIP(9)
      if (('1' <= lookahead && lookahead <= '9')) ADVANCE(226);
      END_STATE();
    case 10:
      if (lookahead == '/') ADVANCE(12);
      if (lookahead == '0') ADVANCE(229);
      if (lookahead == 'm') ADVANCE(36);
      if (lookahead == '\t' ||
          lookahead == '\n' ||
          lookahead == '\r' ||
          lookahead == ' ') SKIP(10)
      if (('1' <= lookahead && lookahead <= '9')) ADVANCE(231);
      END_STATE();
    case 11:
      if (lookahead == '/') ADVANCE(12);
      if (lookahead == ';') ADVANCE(166);
      if (lookahead == 'o') ADVANCE(331);
      if (lookahead == '}') ADVANCE(176);
      if (lookahead == '\t' ||
          lookahead == '\n' ||
          lookahead == '\r' ||
          lookahead == ' ') SKIP(11)
      if (('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 12:
      if (lookahead == '/') ADVANCE(162);
      END_STATE();
    case 13:
      if (lookahead == '2') ADVANCE(196);
      END_STATE();
    case 14:
      if (lookahead == '2') ADVANCE(204);
      END_STATE();
    case 15:
      if (lookahead == '2') ADVANCE(200);
      END_STATE();
    case 16:
      if (lookahead == '2') ADVANCE(208);
      END_STATE();
    case 17:
      if (lookahead == '2') ADVANCE(212);
      END_STATE();
    case 18:
      if (lookahead == '3') ADVANCE(13);
      if (lookahead == '6') ADVANCE(24);
      END_STATE();
    case 19:
      if (lookahead == '3') ADVANCE(2);
      END_STATE();
    case 20:
      if (lookahead == '3') ADVANCE(14);
      if (lookahead == '6') ADVANCE(25);
      END_STATE();
    case 21:
      if (lookahead == '3') ADVANCE(15);
      if (lookahead == '6') ADVANCE(26);
      END_STATE();
    case 22:
      if (lookahead == '3') ADVANCE(16);
      if (lookahead == '6') ADVANCE(27);
      END_STATE();
    case 23:
      if (lookahead == '3') ADVANCE(17);
      if (lookahead == '6') ADVANCE(28);
      END_STATE();
    case 24:
      if (lookahead == '4') ADVANCE(198);
      END_STATE();
    case 25:
      if (lookahead == '4') ADVANCE(206);
      END_STATE();
    case 26:
      if (lookahead == '4') ADVANCE(202);
      END_STATE();
    case 27:
      if (lookahead == '4') ADVANCE(210);
      END_STATE();
    case 28:
      if (lookahead == '4') ADVANCE(214);
      END_STATE();
    case 29:
      if (lookahead == 'a') ADVANCE(76);
      if (lookahead == 'i') ADVANCE(133);
      if (lookahead == 'l') ADVANCE(97);
      END_STATE();
    case 30:
      if (lookahead == 'a') ADVANCE(101);
      if (lookahead == 'e') ADVANCE(113);
      END_STATE();
    case 31:
      if (lookahead == 'a') ADVANCE(82);
      END_STATE();
    case 32:
      if (lookahead == 'a') ADVANCE(41);
      END_STATE();
    case 33:
      if (lookahead == 'a') ADVANCE(68);
      END_STATE();
    case 34:
      if (lookahead == 'a') ADVANCE(132);
      END_STATE();
    case 35:
      if (lookahead == 'a') ADVANCE(80);
      END_STATE();
    case 36:
      if (lookahead == 'a') ADVANCE(131);
      END_STATE();
    case 37:
      if (lookahead == 'a') ADVANCE(116);
      END_STATE();
    case 38:
      if (lookahead == 'a') ADVANCE(122);
      END_STATE();
    case 39:
      if (lookahead == 'a') ADVANCE(69);
      END_STATE();
    case 40:
      if (lookahead == 'b') ADVANCE(77);
      END_STATE();
    case 41:
      if (lookahead == 'c') ADVANCE(74);
      END_STATE();
    case 42:
      if (lookahead == 'c') ADVANCE(184);
      END_STATE();
    case 43:
      if (lookahead == 'c') ADVANCE(55);
      END_STATE();
    case 44:
      if (lookahead == 'd') ADVANCE(188);
      END_STATE();
    case 45:
      if (lookahead == 'd') ADVANCE(220);
      END_STATE();
    case 46:
      if (lookahead == 'd') ADVANCE(22);
      END_STATE();
    case 47:
      if (lookahead == 'd') ADVANCE(23);
      END_STATE();
    case 48:
      if (lookahead == 'e') ADVANCE(103);
      if (lookahead == 'p') ADVANCE(42);
      END_STATE();
    case 49:
      if (lookahead == 'e') ADVANCE(105);
      if (lookahead == 'f') ADVANCE(72);
      if (lookahead == 'i') ADVANCE(87);
      if (lookahead == 't') ADVANCE(106);
      if (lookahead == 'y') ADVANCE(88);
      END_STATE();
    case 50:
      if (lookahead == 'e') ADVANCE(46);
      END_STATE();
    case 51:
      if (lookahead == 'e') ADVANCE(241);
      END_STATE();
    case 52:
      if (lookahead == 'e') ADVANCE(251);
      END_STATE();
    case 53:
      if (lookahead == 'e') ADVANCE(181);
      END_STATE();
    case 54:
      if (lookahead == 'e') ADVANCE(167);
      END_STATE();
    case 55:
      if (lookahead == 'e') ADVANCE(183);
      END_STATE();
    case 56:
      if (lookahead == 'e') ADVANCE(44);
      END_STATE();
    case 57:
      if (lookahead == 'e') ADVANCE(45);
      END_STATE();
    case 58:
      if (lookahead == 'e') ADVANCE(111);
      END_STATE();
    case 59:
      if (lookahead == 'e') ADVANCE(93);
      END_STATE();
    case 60:
      if (lookahead == 'e') ADVANCE(38);
      END_STATE();
    case 61:
      if (lookahead == 'e') ADVANCE(35);
      if (lookahead == 'i') ADVANCE(86);
      END_STATE();
    case 62:
      if (lookahead == 'e') ADVANCE(107);
      END_STATE();
    case 63:
      if (lookahead == 'e') ADVANCE(47);
      END_STATE();
    case 64:
      if (lookahead == 'f') ADVANCE(237);
      END_STATE();
    case 65:
      if (lookahead == 'f') ADVANCE(237);
      if (lookahead == 't') ADVANCE(18);
      END_STATE();
    case 66:
      if (lookahead == 'f') ADVANCE(190);
      END_STATE();
    case 67:
      if (lookahead == 'g') ADVANCE(218);
      END_STATE();
    case 68:
      if (lookahead == 'g') ADVANCE(53);
      END_STATE();
    case 69:
      if (lookahead == 'g') ADVANCE(54);
      END_STATE();
    case 70:
      if (lookahead == 'i') ADVANCE(43);
      END_STATE();
    case 71:
      if (lookahead == 'i') ADVANCE(99);
      END_STATE();
    case 72:
      if (lookahead == 'i') ADVANCE(134);
      END_STATE();
    case 73:
      if (lookahead == 'i') ADVANCE(90);
      END_STATE();
    case 74:
      if (lookahead == 'k') ADVANCE(39);
      END_STATE();
    case 75:
      if (lookahead == 'l') ADVANCE(216);
      END_STATE();
    case 76:
      if (lookahead == 'l') ADVANCE(115);
      END_STATE();
    case 77:
      if (lookahead == 'l') ADVANCE(52);
      END_STATE();
    case 78:
      if (lookahead == 'm') ADVANCE(102);
      if (lookahead == 'n') ADVANCE(65);
      END_STATE();
    case 79:
      if (lookahead == 'm') ADVANCE(173);
      END_STATE();
    case 80:
      if (lookahead == 'm') ADVANCE(185);
      END_STATE();
    case 81:
      if (lookahead == 'n') ADVANCE(59);
      if (lookahead == 'p') ADVANCE(119);
      END_STATE();
    case 82:
      if (lookahead == 'n') ADVANCE(239);
      END_STATE();
    case 83:
      if (lookahead == 'n') ADVANCE(169);
      END_STATE();
    case 84:
      if (lookahead == 'n') ADVANCE(126);
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(235);
      END_STATE();
    case 85:
      if (lookahead == 'n') ADVANCE(64);
      END_STATE();
    case 86:
      if (lookahead == 'n') ADVANCE(67);
      END_STATE();
    case 87:
      if (lookahead == 'n') ADVANCE(123);
      END_STATE();
    case 88:
      if (lookahead == 'n') ADVANCE(120);
      END_STATE();
    case 89:
      if (lookahead == 'n') ADVANCE(112);
      END_STATE();
    case 90:
      if (lookahead == 'n') ADVANCE(124);
      END_STATE();
    case 91:
      if (lookahead == 'o') ADVANCE(125);
      END_STATE();
    case 92:
      if (lookahead == 'o') ADVANCE(222);
      if (lookahead == 'r') ADVANCE(128);
      END_STATE();
    case 93:
      if (lookahead == 'o') ADVANCE(66);
      END_STATE();
    case 94:
      if (lookahead == 'o') ADVANCE(19);
      END_STATE();
    case 95:
      if (lookahead == 'o') ADVANCE(96);
      if (lookahead == 'y') ADVANCE(118);
      END_STATE();
    case 96:
      if (lookahead == 'o') ADVANCE(75);
      END_STATE();
    case 97:
      if (lookahead == 'o') ADVANCE(37);
      END_STATE();
    case 98:
      if (lookahead == 'o') ADVANCE(109);
      END_STATE();
    case 99:
      if (lookahead == 'o') ADVANCE(83);
      END_STATE();
    case 100:
      if (lookahead == 'o') ADVANCE(121);
      END_STATE();
    case 101:
      if (lookahead == 'p') ADVANCE(192);
      if (lookahead == 'x') ADVANCE(224);
      END_STATE();
    case 102:
      if (lookahead == 'p') ADVANCE(98);
      END_STATE();
    case 103:
      if (lookahead == 'p') ADVANCE(60);
      if (lookahead == 's') ADVANCE(62);
      if (lookahead == 't') ADVANCE(127);
      END_STATE();
    case 104:
      if (lookahead == 'p') ADVANCE(110);
      END_STATE();
    case 105:
      if (lookahead == 'r') ADVANCE(129);
      END_STATE();
    case 106:
      if (lookahead == 'r') ADVANCE(61);
      END_STATE();
    case 107:
      if (lookahead == 'r') ADVANCE(130);
      END_STATE();
    case 108:
      if (lookahead == 'r') ADVANCE(89);
      END_STATE();
    case 109:
      if (lookahead == 'r') ADVANCE(117);
      END_STATE();
    case 110:
      if (lookahead == 'r') ADVANCE(100);
      END_STATE();
    case 111:
      if (lookahead == 's') ADVANCE(255);
      END_STATE();
    case 112:
      if (lookahead == 's') ADVANCE(187);
      END_STATE();
    case 113:
      if (lookahead == 's') ADVANCE(114);
      END_STATE();
    case 114:
      if (lookahead == 's') ADVANCE(33);
      END_STATE();
    case 115:
      if (lookahead == 's') ADVANCE(51);
      END_STATE();
    case 116:
      if (lookahead == 't') ADVANCE(253);
      END_STATE();
    case 117:
      if (lookahead == 't') ADVANCE(168);
      END_STATE();
    case 118:
      if (lookahead == 't') ADVANCE(58);
      END_STATE();
    case 119:
      if (lookahead == 't') ADVANCE(71);
      END_STATE();
    case 120:
      if (lookahead == 't') ADVANCE(34);
      END_STATE();
    case 121:
      if (lookahead == 't') ADVANCE(94);
      END_STATE();
    case 122:
      if (lookahead == 't') ADVANCE(56);
      END_STATE();
    case 123:
      if (lookahead == 't') ADVANCE(20);
      END_STATE();
    case 124:
      if (lookahead == 't') ADVANCE(21);
      END_STATE();
    case 125:
      if (lookahead == 'u') ADVANCE(40);
      END_STATE();
    case 126:
      if (lookahead == 'u') ADVANCE(79);
      END_STATE();
    case 127:
      if (lookahead == 'u') ADVANCE(108);
      END_STATE();
    case 128:
      if (lookahead == 'u') ADVANCE(51);
      END_STATE();
    case 129:
      if (lookahead == 'v') ADVANCE(70);
      END_STATE();
    case 130:
      if (lookahead == 'v') ADVANCE(57);
      END_STATE();
    case 131:
      if (lookahead == 'x') ADVANCE(224);
      END_STATE();
    case 132:
      if (lookahead == 'x') ADVANCE(163);
      END_STATE();
    case 133:
      if (lookahead == 'x') ADVANCE(50);
      END_STATE();
    case 134:
      if (lookahead == 'x') ADVANCE(63);
      END_STATE();
    case 135:
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(235);
      END_STATE();
    case 136:
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(234);
      END_STATE();
    case 137:
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(236);
      END_STATE();
    case 138:
      if (lookahead == 'E' ||
          lookahead == 'e') ADVANCE(137);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(138);
      END_STATE();
    case 139:
      if (lookahead == 'X' ||
          lookahead == 'x') ADVANCE(147);
      if (('0' <= lookahead && lookahead <= '7')) ADVANCE(143);
      if (aux_sym_strLit_token2_character_set_1(lookahead)) ADVANCE(250);
      END_STATE();
    case 140:
      if (lookahead == 'X' ||
          lookahead == 'x') ADVANCE(148);
      if (('0' <= lookahead && lookahead <= '7')) ADVANCE(144);
      if (aux_sym_strLit_token2_character_set_1(lookahead)) ADVANCE(246);
      END_STATE();
    case 141:
      if (('0' <= lookahead && lookahead <= '7')) ADVANCE(250);
      END_STATE();
    case 142:
      if (('0' <= lookahead && lookahead <= '7')) ADVANCE(246);
      END_STATE();
    case 143:
      if (('0' <= lookahead && lookahead <= '7')) ADVANCE(141);
      END_STATE();
    case 144:
      if (('0' <= lookahead && lookahead <= '7')) ADVANCE(142);
      END_STATE();
    case 145:
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(250);
      END_STATE();
    case 146:
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(246);
      END_STATE();
    case 147:
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(145);
      END_STATE();
    case 148:
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(146);
      END_STATE();
    case 149:
      ACCEPT_TOKEN(ts_builtin_sym_end);
      END_STATE();
    case 150:
      ACCEPT_TOKEN(sym_comment);
      if (lookahead == '"') ADVANCE(162);
      if (lookahead == '\\') ADVANCE(153);
      if (lookahead != 0 &&
          lookahead != '\n') ADVANCE(150);
      END_STATE();
    case 151:
      ACCEPT_TOKEN(sym_comment);
      if (lookahead == '\'') ADVANCE(162);
      if (lookahead == '\\') ADVANCE(152);
      if (lookahead != 0 &&
          lookahead != '\n') ADVANCE(151);
      END_STATE();
    case 152:
      ACCEPT_TOKEN(sym_comment);
      if (lookahead == 'X' ||
          lookahead == 'x') ADVANCE(160);
      if (('0' <= lookahead && lookahead <= '7')) ADVANCE(156);
      if (aux_sym_strLit_token2_character_set_1(lookahead)) ADVANCE(151);
      if (lookahead != 0 &&
          lookahead != '\n') ADVANCE(162);
      END_STATE();
    case 153:
      ACCEPT_TOKEN(sym_comment);
      if (lookahead == 'X' ||
          lookahead == 'x') ADVANCE(161);
      if (('0' <= lookahead && lookahead <= '7')) ADVANCE(157);
      if (aux_sym_strLit_token2_character_set_1(lookahead)) ADVANCE(150);
      if (lookahead != 0 &&
          lookahead != '\n') ADVANCE(162);
      END_STATE();
    case 154:
      ACCEPT_TOKEN(sym_comment);
      if (('0' <= lookahead && lookahead <= '7')) ADVANCE(151);
      if (lookahead != 0 &&
          lookahead != '\n') ADVANCE(162);
      END_STATE();
    case 155:
      ACCEPT_TOKEN(sym_comment);
      if (('0' <= lookahead && lookahead <= '7')) ADVANCE(150);
      if (lookahead != 0 &&
          lookahead != '\n') ADVANCE(162);
      END_STATE();
    case 156:
      ACCEPT_TOKEN(sym_comment);
      if (('0' <= lookahead && lookahead <= '7')) ADVANCE(154);
      if (lookahead != 0 &&
          lookahead != '\n') ADVANCE(162);
      END_STATE();
    case 157:
      ACCEPT_TOKEN(sym_comment);
      if (('0' <= lookahead && lookahead <= '7')) ADVANCE(155);
      if (lookahead != 0 &&
          lookahead != '\n') ADVANCE(162);
      END_STATE();
    case 158:
      ACCEPT_TOKEN(sym_comment);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(151);
      if (lookahead != 0 &&
          lookahead != '\n') ADVANCE(162);
      END_STATE();
    case 159:
      ACCEPT_TOKEN(sym_comment);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(150);
      if (lookahead != 0 &&
          lookahead != '\n') ADVANCE(162);
      END_STATE();
    case 160:
      ACCEPT_TOKEN(sym_comment);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(158);
      if (lookahead != 0 &&
          lookahead != '\n') ADVANCE(162);
      END_STATE();
    case 161:
      ACCEPT_TOKEN(sym_comment);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(159);
      if (lookahead != 0 &&
          lookahead != '\n') ADVANCE(162);
      END_STATE();
    case 162:
      ACCEPT_TOKEN(sym_comment);
      if (lookahead != 0 &&
          lookahead != '\n') ADVANCE(162);
      END_STATE();
    case 163:
      ACCEPT_TOKEN(anon_sym_syntax);
      END_STATE();
    case 164:
      ACCEPT_TOKEN(anon_sym_EQ);
      END_STATE();
    case 165:
      ACCEPT_TOKEN(aux_sym_syntax_token1);
      END_STATE();
    case 166:
      ACCEPT_TOKEN(anon_sym_SEMI);
      END_STATE();
    case 167:
      ACCEPT_TOKEN(anon_sym_package);
      END_STATE();
    case 168:
      ACCEPT_TOKEN(anon_sym_import);
      END_STATE();
    case 169:
      ACCEPT_TOKEN(anon_sym_option);
      END_STATE();
    case 170:
      ACCEPT_TOKEN(anon_sym_option);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 171:
      ACCEPT_TOKEN(anon_sym_LPAREN);
      END_STATE();
    case 172:
      ACCEPT_TOKEN(anon_sym_RPAREN);
      END_STATE();
    case 173:
      ACCEPT_TOKEN(anon_sym_enum);
      END_STATE();
    case 174:
      ACCEPT_TOKEN(anon_sym_enum);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 175:
      ACCEPT_TOKEN(anon_sym_LBRACE);
      END_STATE();
    case 176:
      ACCEPT_TOKEN(anon_sym_RBRACE);
      END_STATE();
    case 177:
      ACCEPT_TOKEN(anon_sym_DASH);
      END_STATE();
    case 178:
      ACCEPT_TOKEN(anon_sym_LBRACK);
      END_STATE();
    case 179:
      ACCEPT_TOKEN(anon_sym_COMMA);
      END_STATE();
    case 180:
      ACCEPT_TOKEN(anon_sym_RBRACK);
      END_STATE();
    case 181:
      ACCEPT_TOKEN(anon_sym_message);
      END_STATE();
    case 182:
      ACCEPT_TOKEN(anon_sym_message);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 183:
      ACCEPT_TOKEN(anon_sym_service);
      END_STATE();
    case 184:
      ACCEPT_TOKEN(anon_sym_rpc);
      END_STATE();
    case 185:
      ACCEPT_TOKEN(anon_sym_stream);
      END_STATE();
    case 186:
      ACCEPT_TOKEN(anon_sym_stream);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 187:
      ACCEPT_TOKEN(anon_sym_returns);
      END_STATE();
    case 188:
      ACCEPT_TOKEN(anon_sym_repeated);
      END_STATE();
    case 189:
      ACCEPT_TOKEN(anon_sym_repeated);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 190:
      ACCEPT_TOKEN(anon_sym_oneof);
      END_STATE();
    case 191:
      ACCEPT_TOKEN(anon_sym_oneof);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 192:
      ACCEPT_TOKEN(anon_sym_map);
      END_STATE();
    case 193:
      ACCEPT_TOKEN(anon_sym_map);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 194:
      ACCEPT_TOKEN(anon_sym_LT);
      END_STATE();
    case 195:
      ACCEPT_TOKEN(anon_sym_GT);
      END_STATE();
    case 196:
      ACCEPT_TOKEN(anon_sym_int32);
      END_STATE();
    case 197:
      ACCEPT_TOKEN(anon_sym_int32);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 198:
      ACCEPT_TOKEN(anon_sym_int64);
      END_STATE();
    case 199:
      ACCEPT_TOKEN(anon_sym_int64);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 200:
      ACCEPT_TOKEN(anon_sym_uint32);
      END_STATE();
    case 201:
      ACCEPT_TOKEN(anon_sym_uint32);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 202:
      ACCEPT_TOKEN(anon_sym_uint64);
      END_STATE();
    case 203:
      ACCEPT_TOKEN(anon_sym_uint64);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 204:
      ACCEPT_TOKEN(anon_sym_sint32);
      END_STATE();
    case 205:
      ACCEPT_TOKEN(anon_sym_sint32);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 206:
      ACCEPT_TOKEN(anon_sym_sint64);
      END_STATE();
    case 207:
      ACCEPT_TOKEN(anon_sym_sint64);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 208:
      ACCEPT_TOKEN(anon_sym_fixed32);
      END_STATE();
    case 209:
      ACCEPT_TOKEN(anon_sym_fixed32);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 210:
      ACCEPT_TOKEN(anon_sym_fixed64);
      END_STATE();
    case 211:
      ACCEPT_TOKEN(anon_sym_fixed64);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 212:
      ACCEPT_TOKEN(anon_sym_sfixed32);
      END_STATE();
    case 213:
      ACCEPT_TOKEN(anon_sym_sfixed32);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 214:
      ACCEPT_TOKEN(anon_sym_sfixed64);
      END_STATE();
    case 215:
      ACCEPT_TOKEN(anon_sym_sfixed64);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 216:
      ACCEPT_TOKEN(anon_sym_bool);
      END_STATE();
    case 217:
      ACCEPT_TOKEN(anon_sym_bool);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 218:
      ACCEPT_TOKEN(anon_sym_string);
      END_STATE();
    case 219:
      ACCEPT_TOKEN(anon_sym_string);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 220:
      ACCEPT_TOKEN(anon_sym_reserved);
      END_STATE();
    case 221:
      ACCEPT_TOKEN(anon_sym_reserved);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 222:
      ACCEPT_TOKEN(anon_sym_to);
      END_STATE();
    case 223:
      ACCEPT_TOKEN(anon_sym_to);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 224:
      ACCEPT_TOKEN(anon_sym_max);
      END_STATE();
    case 225:
      ACCEPT_TOKEN(sym_intLit);
      if (lookahead == '.') ADVANCE(233);
      if (lookahead == 'E' ||
          lookahead == 'e') ADVANCE(135);
      if (lookahead == 'X' ||
          lookahead == 'x') ADVANCE(232);
      if (lookahead == '8' ||
          lookahead == '9') ADVANCE(228);
      if (('0' <= lookahead && lookahead <= '7')) ADVANCE(227);
      END_STATE();
    case 226:
      ACCEPT_TOKEN(sym_intLit);
      if (lookahead == '.') ADVANCE(233);
      if (lookahead == 'E' ||
          lookahead == 'e') ADVANCE(135);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(228);
      END_STATE();
    case 227:
      ACCEPT_TOKEN(sym_intLit);
      if (lookahead == 'E' ||
          lookahead == 'e') ADVANCE(135);
      if (lookahead == '8' ||
          lookahead == '9') ADVANCE(228);
      if (('0' <= lookahead && lookahead <= '7')) ADVANCE(227);
      END_STATE();
    case 228:
      ACCEPT_TOKEN(sym_intLit);
      if (lookahead == 'E' ||
          lookahead == 'e') ADVANCE(135);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(228);
      END_STATE();
    case 229:
      ACCEPT_TOKEN(sym_intLit);
      if (lookahead == 'X' ||
          lookahead == 'x') ADVANCE(232);
      if (lookahead == '8' ||
          lookahead == '9') ADVANCE(231);
      if (('0' <= lookahead && lookahead <= '7')) ADVANCE(230);
      END_STATE();
    case 230:
      ACCEPT_TOKEN(sym_intLit);
      if (lookahead == '8' ||
          lookahead == '9') ADVANCE(231);
      if (('0' <= lookahead && lookahead <= '7')) ADVANCE(230);
      END_STATE();
    case 231:
      ACCEPT_TOKEN(sym_intLit);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(231);
      END_STATE();
    case 232:
      ACCEPT_TOKEN(sym_intLit);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(232);
      END_STATE();
    case 233:
      ACCEPT_TOKEN(aux_sym_floatLit_token1);
      if (lookahead == 'E' ||
          lookahead == 'e') ADVANCE(136);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(233);
      END_STATE();
    case 234:
      ACCEPT_TOKEN(aux_sym_floatLit_token1);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(234);
      END_STATE();
    case 235:
      ACCEPT_TOKEN(aux_sym_floatLit_token2);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(235);
      END_STATE();
    case 236:
      ACCEPT_TOKEN(aux_sym_floatLit_token3);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(236);
      END_STATE();
    case 237:
      ACCEPT_TOKEN(anon_sym_inf);
      END_STATE();
    case 238:
      ACCEPT_TOKEN(anon_sym_inf);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 239:
      ACCEPT_TOKEN(anon_sym_nan);
      END_STATE();
    case 240:
      ACCEPT_TOKEN(anon_sym_nan);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 241:
      ACCEPT_TOKEN(sym_boolLit);
      END_STATE();
    case 242:
      ACCEPT_TOKEN(sym_boolLit);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 243:
      ACCEPT_TOKEN(anon_sym_DQUOTE);
      END_STATE();
    case 244:
      ACCEPT_TOKEN(aux_sym_strLit_token1);
      if (lookahead == '/') ADVANCE(245);
      if (lookahead == '\\') ADVANCE(140);
      if (lookahead == '\t' ||
          lookahead == '\r' ||
          lookahead == ' ') ADVANCE(244);
      if (lookahead != 0 &&
          lookahead != '\n' &&
          lookahead != '"') ADVANCE(246);
      END_STATE();
    case 245:
      ACCEPT_TOKEN(aux_sym_strLit_token1);
      if (lookahead == '/') ADVANCE(150);
      if (lookahead == '\\') ADVANCE(140);
      if (lookahead != 0 &&
          lookahead != '\n' &&
          lookahead != '"') ADVANCE(246);
      END_STATE();
    case 246:
      ACCEPT_TOKEN(aux_sym_strLit_token1);
      if (lookahead == '\\') ADVANCE(140);
      if (lookahead != 0 &&
          lookahead != '\n' &&
          lookahead != '"') ADVANCE(246);
      END_STATE();
    case 247:
      ACCEPT_TOKEN(anon_sym_SQUOTE);
      END_STATE();
    case 248:
      ACCEPT_TOKEN(aux_sym_strLit_token2);
      if (lookahead == '/') ADVANCE(249);
      if (lookahead == '\\') ADVANCE(139);
      if (lookahead == '\t' ||
          lookahead == '\r' ||
          lookahead == ' ') ADVANCE(248);
      if (lookahead != 0 &&
          lookahead != '\n' &&
          lookahead != '\'') ADVANCE(250);
      END_STATE();
    case 249:
      ACCEPT_TOKEN(aux_sym_strLit_token2);
      if (lookahead == '/') ADVANCE(151);
      if (lookahead == '\\') ADVANCE(139);
      if (lookahead != 0 &&
          lookahead != '\n' &&
          lookahead != '\'') ADVANCE(250);
      END_STATE();
    case 250:
      ACCEPT_TOKEN(aux_sym_strLit_token2);
      if (lookahead == '\\') ADVANCE(139);
      if (lookahead != 0 &&
          lookahead != '\n' &&
          lookahead != '\'') ADVANCE(250);
      END_STATE();
    case 251:
      ACCEPT_TOKEN(anon_sym_double);
      END_STATE();
    case 252:
      ACCEPT_TOKEN(anon_sym_double);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 253:
      ACCEPT_TOKEN(anon_sym_float);
      END_STATE();
    case 254:
      ACCEPT_TOKEN(anon_sym_float);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 255:
      ACCEPT_TOKEN(anon_sym_bytes);
      END_STATE();
    case 256:
      ACCEPT_TOKEN(anon_sym_bytes);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 257:
      ACCEPT_TOKEN(aux_sym_constant_token1);
      END_STATE();
    case 258:
      ACCEPT_TOKEN(anon_sym_COLON);
      END_STATE();
    case 259:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == '2') ADVANCE(197);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 260:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == '2') ADVANCE(205);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 261:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == '2') ADVANCE(201);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 262:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == '2') ADVANCE(209);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 263:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == '2') ADVANCE(213);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 264:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == '3') ADVANCE(259);
      if (lookahead == '6') ADVANCE(269);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 265:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == '3') ADVANCE(260);
      if (lookahead == '6') ADVANCE(270);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 266:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == '3') ADVANCE(261);
      if (lookahead == '6') ADVANCE(271);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 267:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == '3') ADVANCE(262);
      if (lookahead == '6') ADVANCE(272);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 268:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == '3') ADVANCE(263);
      if (lookahead == '6') ADVANCE(273);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 269:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == '4') ADVANCE(199);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 270:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == '4') ADVANCE(207);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 271:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == '4') ADVANCE(203);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 272:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == '4') ADVANCE(211);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 273:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == '4') ADVANCE(215);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 274:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'a') ADVANCE(330);
      if (lookahead == 'e') ADVANCE(338);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('b' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 275:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'a') ADVANCE(303);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('b' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 276:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'a') ADVANCE(313);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('b' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 277:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'a') ADVANCE(310);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('b' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 278:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'a') ADVANCE(342);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('b' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 279:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'a') ADVANCE(315);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('b' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 280:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'a') ADVANCE(346);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('b' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 281:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'b') ADVANCE(311);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 282:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'd') ADVANCE(189);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 283:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'd') ADVANCE(221);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 284:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'd') ADVANCE(267);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 285:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'd') ADVANCE(268);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 286:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'e') ADVANCE(332);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 287:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'e') ADVANCE(284);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 288:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'e') ADVANCE(252);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 289:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'e') ADVANCE(182);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 290:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'e') ADVANCE(242);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 291:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'e') ADVANCE(333);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 292:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'e') ADVANCE(282);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 293:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'e') ADVANCE(337);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 294:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'e') ADVANCE(280);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 295:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'e') ADVANCE(283);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 296:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'e') ADVANCE(326);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 297:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'e') ADVANCE(276);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 298:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'e') ADVANCE(285);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 299:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'f') ADVANCE(191);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 300:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'f') ADVANCE(238);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 301:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'f') ADVANCE(308);
      if (lookahead == 'i') ADVANCE(321);
      if (lookahead == 't') ADVANCE(336);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 302:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'g') ADVANCE(219);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 303:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'g') ADVANCE(289);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 304:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'i') ADVANCE(353);
      if (lookahead == 'l') ADVANCE(325);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 305:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'i') ADVANCE(320);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 306:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'i') ADVANCE(329);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 307:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'i') ADVANCE(322);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 308:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'i') ADVANCE(354);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 309:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'l') ADVANCE(217);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 310:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'l') ADVANCE(340);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 311:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'l') ADVANCE(288);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 312:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'm') ADVANCE(174);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 313:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'm') ADVANCE(186);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 314:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'n') ADVANCE(170);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 315:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'n') ADVANCE(240);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 316:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'n') ADVANCE(296);
      if (lookahead == 'p') ADVANCE(344);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 317:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'n') ADVANCE(341);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 318:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'n') ADVANCE(350);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 319:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'n') ADVANCE(300);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 320:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'n') ADVANCE(302);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 321:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'n') ADVANCE(347);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 322:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'n') ADVANCE(348);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 323:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'o') ADVANCE(349);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 324:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'o') ADVANCE(309);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 325:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'o') ADVANCE(278);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 326:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'o') ADVANCE(299);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 327:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'o') ADVANCE(223);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 328:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'o') ADVANCE(324);
      if (lookahead == 'y') ADVANCE(343);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 329:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'o') ADVANCE(314);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 330:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'p') ADVANCE(193);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 331:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'p') ADVANCE(344);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 332:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'p') ADVANCE(294);
      if (lookahead == 's') ADVANCE(291);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 333:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'r') ADVANCE(352);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 334:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'r') ADVANCE(351);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 335:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'r') ADVANCE(297);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 336:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'r') ADVANCE(305);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 337:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 's') ADVANCE(256);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 338:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 's') ADVANCE(339);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 339:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 's') ADVANCE(275);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 340:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 's') ADVANCE(290);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 341:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 't') ADVANCE(264);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 342:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 't') ADVANCE(254);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 343:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 't') ADVANCE(293);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 344:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 't') ADVANCE(306);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 345:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 't') ADVANCE(335);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 346:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 't') ADVANCE(292);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 347:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 't') ADVANCE(265);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 348:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 't') ADVANCE(266);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 349:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'u') ADVANCE(281);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 350:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'u') ADVANCE(312);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 351:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'u') ADVANCE(290);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 352:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'v') ADVANCE(295);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 353:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'x') ADVANCE(287);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 354:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == 'x') ADVANCE(298);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 355:
      ACCEPT_TOKEN(sym_ident);
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(235);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 356:
      ACCEPT_TOKEN(sym_ident);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(356);
      END_STATE();
    case 357:
      ACCEPT_TOKEN(anon_sym_DOT);
      END_STATE();
    default:
      return false;
  }
}

static const TSLexMode ts_lex_modes[STATE_COUNT] = {
  [0] = {.lex_state = 0},
  [1] = {.lex_state = 0},
  [2] = {.lex_state = 3},
  [3] = {.lex_state = 3},
  [4] = {.lex_state = 3},
  [5] = {.lex_state = 3},
  [6] = {.lex_state = 3},
  [7] = {.lex_state = 3},
  [8] = {.lex_state = 5},
  [9] = {.lex_state = 3},
  [10] = {.lex_state = 3},
  [11] = {.lex_state = 6},
  [12] = {.lex_state = 3},
  [13] = {.lex_state = 3},
  [14] = {.lex_state = 3},
  [15] = {.lex_state = 6},
  [16] = {.lex_state = 6},
  [17] = {.lex_state = 3},
  [18] = {.lex_state = 3},
  [19] = {.lex_state = 3},
  [20] = {.lex_state = 3},
  [21] = {.lex_state = 3},
  [22] = {.lex_state = 3},
  [23] = {.lex_state = 3},
  [24] = {.lex_state = 3},
  [25] = {.lex_state = 3},
  [26] = {.lex_state = 3},
  [27] = {.lex_state = 3},
  [28] = {.lex_state = 3},
  [29] = {.lex_state = 3},
  [30] = {.lex_state = 3},
  [31] = {.lex_state = 3},
  [32] = {.lex_state = 3},
  [33] = {.lex_state = 3},
  [34] = {.lex_state = 3},
  [35] = {.lex_state = 3},
  [36] = {.lex_state = 3},
  [37] = {.lex_state = 3},
  [38] = {.lex_state = 7},
  [39] = {.lex_state = 7},
  [40] = {.lex_state = 6},
  [41] = {.lex_state = 6},
  [42] = {.lex_state = 6},
  [43] = {.lex_state = 6},
  [44] = {.lex_state = 1},
  [45] = {.lex_state = 1},
  [46] = {.lex_state = 1},
  [47] = {.lex_state = 1},
  [48] = {.lex_state = 1},
  [49] = {.lex_state = 1},
  [50] = {.lex_state = 1},
  [51] = {.lex_state = 0},
  [52] = {.lex_state = 0},
  [53] = {.lex_state = 0},
  [54] = {.lex_state = 0},
  [55] = {.lex_state = 0},
  [56] = {.lex_state = 0},
  [57] = {.lex_state = 4},
  [58] = {.lex_state = 4},
  [59] = {.lex_state = 4},
  [60] = {.lex_state = 4},
  [61] = {.lex_state = 0},
  [62] = {.lex_state = 0},
  [63] = {.lex_state = 0},
  [64] = {.lex_state = 11},
  [65] = {.lex_state = 0},
  [66] = {.lex_state = 0},
  [67] = {.lex_state = 0},
  [68] = {.lex_state = 0},
  [69] = {.lex_state = 11},
  [70] = {.lex_state = 0},
  [71] = {.lex_state = 11},
  [72] = {.lex_state = 0},
  [73] = {.lex_state = 0},
  [74] = {.lex_state = 11},
  [75] = {.lex_state = 0},
  [76] = {.lex_state = 0},
  [77] = {.lex_state = 0},
  [78] = {.lex_state = 0},
  [79] = {.lex_state = 11},
  [80] = {.lex_state = 9},
  [81] = {.lex_state = 0},
  [82] = {.lex_state = 4},
  [83] = {.lex_state = 8},
  [84] = {.lex_state = 8},
  [85] = {.lex_state = 0},
  [86] = {.lex_state = 0},
  [87] = {.lex_state = 4},
  [88] = {.lex_state = 4},
  [89] = {.lex_state = 0},
  [90] = {.lex_state = 0},
  [91] = {.lex_state = 4},
  [92] = {.lex_state = 0},
  [93] = {.lex_state = 4},
  [94] = {.lex_state = 8},
  [95] = {.lex_state = 0},
  [96] = {.lex_state = 4},
  [97] = {.lex_state = 4},
  [98] = {.lex_state = 4},
  [99] = {.lex_state = 4},
  [100] = {.lex_state = 4},
  [101] = {.lex_state = 4},
  [102] = {.lex_state = 4},
  [103] = {.lex_state = 4},
  [104] = {.lex_state = 4},
  [105] = {.lex_state = 4},
  [106] = {.lex_state = 4},
  [107] = {.lex_state = 4},
  [108] = {.lex_state = 4},
  [109] = {.lex_state = 11},
  [110] = {.lex_state = 4},
  [111] = {.lex_state = 4},
  [112] = {.lex_state = 0},
  [113] = {.lex_state = 0},
  [114] = {.lex_state = 0},
  [115] = {.lex_state = 0},
  [116] = {.lex_state = 0},
  [117] = {.lex_state = 11},
  [118] = {.lex_state = 4},
  [119] = {.lex_state = 11},
  [120] = {.lex_state = 11},
  [121] = {.lex_state = 4},
  [122] = {.lex_state = 4},
  [123] = {.lex_state = 11},
  [124] = {.lex_state = 11},
  [125] = {.lex_state = 4},
  [126] = {.lex_state = 11},
  [127] = {.lex_state = 4},
  [128] = {.lex_state = 4},
  [129] = {.lex_state = 0},
  [130] = {.lex_state = 4},
  [131] = {.lex_state = 0},
  [132] = {.lex_state = 0},
  [133] = {.lex_state = 0},
  [134] = {.lex_state = 0},
  [135] = {.lex_state = 4},
  [136] = {.lex_state = 4},
  [137] = {.lex_state = 0},
  [138] = {.lex_state = 0},
  [139] = {.lex_state = 0},
  [140] = {.lex_state = 4},
  [141] = {.lex_state = 4},
  [142] = {.lex_state = 4},
  [143] = {.lex_state = 0},
  [144] = {.lex_state = 4},
  [145] = {.lex_state = 0},
  [146] = {.lex_state = 3},
  [147] = {.lex_state = 4},
  [148] = {.lex_state = 0},
  [149] = {.lex_state = 0},
  [150] = {.lex_state = 4},
  [151] = {.lex_state = 4},
  [152] = {.lex_state = 0},
  [153] = {.lex_state = 4},
  [154] = {.lex_state = 4},
  [155] = {.lex_state = 4},
  [156] = {.lex_state = 4},
  [157] = {.lex_state = 0},
  [158] = {.lex_state = 4},
  [159] = {.lex_state = 0},
  [160] = {.lex_state = 3},
  [161] = {.lex_state = 4},
  [162] = {.lex_state = 4},
  [163] = {.lex_state = 0},
  [164] = {.lex_state = 4},
  [165] = {.lex_state = 0},
  [166] = {.lex_state = 0},
  [167] = {.lex_state = 0},
  [168] = {.lex_state = 0},
  [169] = {.lex_state = 10},
  [170] = {.lex_state = 0},
  [171] = {.lex_state = 3},
  [172] = {.lex_state = 0},
  [173] = {.lex_state = 4},
  [174] = {.lex_state = 0},
  [175] = {.lex_state = 3},
  [176] = {.lex_state = 0},
  [177] = {.lex_state = 4},
  [178] = {.lex_state = 3},
  [179] = {.lex_state = 0},
  [180] = {.lex_state = 0},
  [181] = {.lex_state = 4},
  [182] = {.lex_state = 4},
  [183] = {.lex_state = 3},
  [184] = {.lex_state = 0},
  [185] = {.lex_state = 4},
  [186] = {.lex_state = 0},
  [187] = {.lex_state = 0},
  [188] = {.lex_state = 0},
  [189] = {.lex_state = 0},
  [190] = {.lex_state = 0},
  [191] = {.lex_state = 0},
  [192] = {.lex_state = 0},
  [193] = {.lex_state = 0},
  [194] = {.lex_state = 0},
  [195] = {.lex_state = 0},
  [196] = {.lex_state = 0},
  [197] = {.lex_state = 0},
  [198] = {.lex_state = 0},
  [199] = {.lex_state = 0},
  [200] = {.lex_state = 0},
  [201] = {.lex_state = 0},
  [202] = {.lex_state = 4},
  [203] = {.lex_state = 0},
  [204] = {.lex_state = 0},
  [205] = {.lex_state = 0},
  [206] = {.lex_state = 0},
  [207] = {.lex_state = 0},
  [208] = {.lex_state = 0},
  [209] = {.lex_state = 0},
  [210] = {.lex_state = 4},
  [211] = {.lex_state = 0},
  [212] = {.lex_state = 0},
  [213] = {.lex_state = 0},
  [214] = {.lex_state = 0},
  [215] = {.lex_state = 0},
  [216] = {.lex_state = 0},
  [217] = {.lex_state = 0},
  [218] = {.lex_state = 0},
  [219] = {.lex_state = 0},
  [220] = {.lex_state = 0},
  [221] = {.lex_state = 0},
  [222] = {.lex_state = 0},
  [223] = {.lex_state = 0},
  [224] = {.lex_state = 0},
  [225] = {.lex_state = 0},
  [226] = {.lex_state = 0},
  [227] = {.lex_state = 248},
  [228] = {.lex_state = 244},
  [229] = {.lex_state = 0},
  [230] = {.lex_state = 0},
  [231] = {.lex_state = 0},
  [232] = {.lex_state = 0},
  [233] = {.lex_state = 0},
  [234] = {.lex_state = 0},
  [235] = {.lex_state = 0},
  [236] = {.lex_state = 0},
  [237] = {.lex_state = 3},
  [238] = {.lex_state = 0},
  [239] = {.lex_state = 0},
  [240] = {.lex_state = 0},
  [241] = {.lex_state = 0},
  [242] = {.lex_state = 0},
  [243] = {.lex_state = 0},
  [244] = {.lex_state = 3},
  [245] = {.lex_state = 0},
  [246] = {.lex_state = 0},
  [247] = {.lex_state = 0},
  [248] = {.lex_state = 0},
  [249] = {.lex_state = 0},
  [250] = {.lex_state = 0},
  [251] = {.lex_state = 0},
};

static const uint16_t ts_parse_table[LARGE_STATE_COUNT][SYMBOL_COUNT] = {
  [0] = {
    [ts_builtin_sym_end] = ACTIONS(1),
    [sym_comment] = ACTIONS(3),
    [anon_sym_syntax] = ACTIONS(1),
    [anon_sym_EQ] = ACTIONS(1),
    [anon_sym_SEMI] = ACTIONS(1),
    [anon_sym_package] = ACTIONS(1),
    [anon_sym_import] = ACTIONS(1),
    [anon_sym_option] = ACTIONS(1),
    [anon_sym_LPAREN] = ACTIONS(1),
    [anon_sym_RPAREN] = ACTIONS(1),
    [anon_sym_enum] = ACTIONS(1),
    [anon_sym_LBRACE] = ACTIONS(1),
    [anon_sym_RBRACE] = ACTIONS(1),
    [anon_sym_DASH] = ACTIONS(1),
    [anon_sym_LBRACK] = ACTIONS(1),
    [anon_sym_COMMA] = ACTIONS(1),
    [anon_sym_RBRACK] = ACTIONS(1),
    [anon_sym_message] = ACTIONS(1),
    [anon_sym_service] = ACTIONS(1),
    [anon_sym_rpc] = ACTIONS(1),
    [anon_sym_stream] = ACTIONS(1),
    [anon_sym_returns] = ACTIONS(1),
    [anon_sym_repeated] = ACTIONS(1),
    [anon_sym_oneof] = ACTIONS(1),
    [anon_sym_map] = ACTIONS(1),
    [anon_sym_LT] = ACTIONS(1),
    [anon_sym_GT] = ACTIONS(1),
    [anon_sym_int32] = ACTIONS(1),
    [anon_sym_int64] = ACTIONS(1),
    [anon_sym_uint32] = ACTIONS(1),
    [anon_sym_uint64] = ACTIONS(1),
    [anon_sym_sint32] = ACTIONS(1),
    [anon_sym_sint64] = ACTIONS(1),
    [anon_sym_fixed32] = ACTIONS(1),
    [anon_sym_fixed64] = ACTIONS(1),
    [anon_sym_sfixed32] = ACTIONS(1),
    [anon_sym_sfixed64] = ACTIONS(1),
    [anon_sym_bool] = ACTIONS(1),
    [anon_sym_string] = ACTIONS(1),
    [anon_sym_reserved] = ACTIONS(1),
    [anon_sym_to] = ACTIONS(1),
    [anon_sym_max] = ACTIONS(1),
    [sym_intLit] = ACTIONS(1),
    [aux_sym_floatLit_token1] = ACTIONS(1),
    [aux_sym_floatLit_token2] = ACTIONS(1),
    [anon_sym_inf] = ACTIONS(1),
    [anon_sym_nan] = ACTIONS(1),
    [sym_boolLit] = ACTIONS(1),
    [anon_sym_DQUOTE] = ACTIONS(1),
    [anon_sym_SQUOTE] = ACTIONS(1),
    [anon_sym_double] = ACTIONS(1),
    [anon_sym_float] = ACTIONS(1),
    [anon_sym_bytes] = ACTIONS(1),
    [aux_sym_constant_token1] = ACTIONS(1),
    [anon_sym_COLON] = ACTIONS(1),
    [anon_sym_DOT] = ACTIONS(1),
  },
  [1] = {
    [sym_source_file] = STATE(245),
    [sym_syntax] = STATE(52),
    [sym_comment] = ACTIONS(3),
    [anon_sym_syntax] = ACTIONS(5),
  },
};

static const uint16_t ts_small_parse_table[] = {
  [0] = 18,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(7), 1,
      anon_sym_SEMI,
    ACTIONS(9), 1,
      anon_sym_option,
    ACTIONS(11), 1,
      anon_sym_enum,
    ACTIONS(13), 1,
      anon_sym_RBRACE,
    ACTIONS(15), 1,
      anon_sym_message,
    ACTIONS(17), 1,
      anon_sym_repeated,
    ACTIONS(19), 1,
      anon_sym_oneof,
    ACTIONS(21), 1,
      anon_sym_map,
    ACTIONS(25), 1,
      anon_sym_reserved,
    ACTIONS(27), 1,
      sym_ident,
    ACTIONS(29), 1,
      anon_sym_DOT,
    STATE(130), 1,
      sym_messageName,
    STATE(142), 1,
      aux_sym_enumMessageType_repeat1,
    STATE(144), 1,
      sym_enumMessageType,
    STATE(158), 1,
      sym_type,
    STATE(4), 9,
      sym_option,
      sym_enum,
      sym_message,
      sym_field,
      sym_oneof,
      sym_mapField,
      sym_reserved,
      sym_emptyStatement,
      aux_sym_messageBody_repeat1,
    ACTIONS(23), 15,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
  [77] = 18,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(31), 1,
      anon_sym_SEMI,
    ACTIONS(34), 1,
      anon_sym_option,
    ACTIONS(37), 1,
      anon_sym_enum,
    ACTIONS(40), 1,
      anon_sym_RBRACE,
    ACTIONS(42), 1,
      anon_sym_message,
    ACTIONS(45), 1,
      anon_sym_repeated,
    ACTIONS(48), 1,
      anon_sym_oneof,
    ACTIONS(51), 1,
      anon_sym_map,
    ACTIONS(57), 1,
      anon_sym_reserved,
    ACTIONS(60), 1,
      sym_ident,
    ACTIONS(63), 1,
      anon_sym_DOT,
    STATE(130), 1,
      sym_messageName,
    STATE(142), 1,
      aux_sym_enumMessageType_repeat1,
    STATE(144), 1,
      sym_enumMessageType,
    STATE(158), 1,
      sym_type,
    STATE(3), 9,
      sym_option,
      sym_enum,
      sym_message,
      sym_field,
      sym_oneof,
      sym_mapField,
      sym_reserved,
      sym_emptyStatement,
      aux_sym_messageBody_repeat1,
    ACTIONS(54), 15,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
  [154] = 18,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(7), 1,
      anon_sym_SEMI,
    ACTIONS(9), 1,
      anon_sym_option,
    ACTIONS(11), 1,
      anon_sym_enum,
    ACTIONS(15), 1,
      anon_sym_message,
    ACTIONS(17), 1,
      anon_sym_repeated,
    ACTIONS(19), 1,
      anon_sym_oneof,
    ACTIONS(21), 1,
      anon_sym_map,
    ACTIONS(25), 1,
      anon_sym_reserved,
    ACTIONS(27), 1,
      sym_ident,
    ACTIONS(29), 1,
      anon_sym_DOT,
    ACTIONS(66), 1,
      anon_sym_RBRACE,
    STATE(130), 1,
      sym_messageName,
    STATE(142), 1,
      aux_sym_enumMessageType_repeat1,
    STATE(144), 1,
      sym_enumMessageType,
    STATE(158), 1,
      sym_type,
    STATE(3), 9,
      sym_option,
      sym_enum,
      sym_message,
      sym_field,
      sym_oneof,
      sym_mapField,
      sym_reserved,
      sym_emptyStatement,
      aux_sym_messageBody_repeat1,
    ACTIONS(23), 15,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
  [231] = 18,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(7), 1,
      anon_sym_SEMI,
    ACTIONS(9), 1,
      anon_sym_option,
    ACTIONS(11), 1,
      anon_sym_enum,
    ACTIONS(15), 1,
      anon_sym_message,
    ACTIONS(17), 1,
      anon_sym_repeated,
    ACTIONS(19), 1,
      anon_sym_oneof,
    ACTIONS(21), 1,
      anon_sym_map,
    ACTIONS(25), 1,
      anon_sym_reserved,
    ACTIONS(27), 1,
      sym_ident,
    ACTIONS(29), 1,
      anon_sym_DOT,
    ACTIONS(68), 1,
      anon_sym_RBRACE,
    STATE(130), 1,
      sym_messageName,
    STATE(142), 1,
      aux_sym_enumMessageType_repeat1,
    STATE(144), 1,
      sym_enumMessageType,
    STATE(158), 1,
      sym_type,
    STATE(6), 9,
      sym_option,
      sym_enum,
      sym_message,
      sym_field,
      sym_oneof,
      sym_mapField,
      sym_reserved,
      sym_emptyStatement,
      aux_sym_messageBody_repeat1,
    ACTIONS(23), 15,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
  [308] = 18,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(7), 1,
      anon_sym_SEMI,
    ACTIONS(9), 1,
      anon_sym_option,
    ACTIONS(11), 1,
      anon_sym_enum,
    ACTIONS(15), 1,
      anon_sym_message,
    ACTIONS(17), 1,
      anon_sym_repeated,
    ACTIONS(19), 1,
      anon_sym_oneof,
    ACTIONS(21), 1,
      anon_sym_map,
    ACTIONS(25), 1,
      anon_sym_reserved,
    ACTIONS(27), 1,
      sym_ident,
    ACTIONS(29), 1,
      anon_sym_DOT,
    ACTIONS(70), 1,
      anon_sym_RBRACE,
    STATE(130), 1,
      sym_messageName,
    STATE(142), 1,
      aux_sym_enumMessageType_repeat1,
    STATE(144), 1,
      sym_enumMessageType,
    STATE(158), 1,
      sym_type,
    STATE(3), 9,
      sym_option,
      sym_enum,
      sym_message,
      sym_field,
      sym_oneof,
      sym_mapField,
      sym_reserved,
      sym_emptyStatement,
      aux_sym_messageBody_repeat1,
    ACTIONS(23), 15,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
  [385] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(72), 6,
      anon_sym_EQ,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_COMMA,
      anon_sym_COLON,
      anon_sym_DOT,
    ACTIONS(74), 23,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_repeated,
      anon_sym_oneof,
      anon_sym_map,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_reserved,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [422] = 4,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(80), 1,
      anon_sym_to,
    ACTIONS(76), 4,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_COMMA,
      anon_sym_DOT,
    ACTIONS(78), 23,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_repeated,
      anon_sym_oneof,
      anon_sym_map,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_reserved,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [460] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(86), 1,
      anon_sym_COMMA,
    STATE(13), 1,
      aux_sym_fieldNames_repeat1,
    ACTIONS(82), 3,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_DOT,
    ACTIONS(84), 23,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_repeated,
      anon_sym_oneof,
      anon_sym_map,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_reserved,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [500] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(92), 1,
      anon_sym_COMMA,
    STATE(17), 1,
      aux_sym_ranges_repeat1,
    ACTIONS(88), 3,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_DOT,
    ACTIONS(90), 23,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_repeated,
      anon_sym_oneof,
      anon_sym_map,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_reserved,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [540] = 12,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(27), 1,
      sym_ident,
    ACTIONS(29), 1,
      anon_sym_DOT,
    ACTIONS(94), 1,
      anon_sym_SEMI,
    ACTIONS(96), 1,
      anon_sym_option,
    ACTIONS(98), 1,
      anon_sym_RBRACE,
    STATE(130), 1,
      sym_messageName,
    STATE(142), 1,
      aux_sym_enumMessageType_repeat1,
    STATE(144), 1,
      sym_enumMessageType,
    STATE(177), 1,
      sym_type,
    STATE(15), 4,
      sym_option,
      sym_oneofField,
      sym_emptyStatement,
      aux_sym_oneof_repeat1,
    ACTIONS(23), 15,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
  [594] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(104), 1,
      anon_sym_COMMA,
    STATE(12), 1,
      aux_sym_ranges_repeat1,
    ACTIONS(100), 3,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_DOT,
    ACTIONS(102), 23,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_repeated,
      anon_sym_oneof,
      anon_sym_map,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_reserved,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [634] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(86), 1,
      anon_sym_COMMA,
    STATE(14), 1,
      aux_sym_fieldNames_repeat1,
    ACTIONS(107), 3,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_DOT,
    ACTIONS(109), 23,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_repeated,
      anon_sym_oneof,
      anon_sym_map,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_reserved,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [674] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(115), 1,
      anon_sym_COMMA,
    STATE(14), 1,
      aux_sym_fieldNames_repeat1,
    ACTIONS(111), 3,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_DOT,
    ACTIONS(113), 23,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_repeated,
      anon_sym_oneof,
      anon_sym_map,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_reserved,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [714] = 12,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(118), 1,
      anon_sym_SEMI,
    ACTIONS(121), 1,
      anon_sym_option,
    ACTIONS(124), 1,
      anon_sym_RBRACE,
    ACTIONS(129), 1,
      sym_ident,
    ACTIONS(132), 1,
      anon_sym_DOT,
    STATE(130), 1,
      sym_messageName,
    STATE(142), 1,
      aux_sym_enumMessageType_repeat1,
    STATE(144), 1,
      sym_enumMessageType,
    STATE(177), 1,
      sym_type,
    STATE(15), 4,
      sym_option,
      sym_oneofField,
      sym_emptyStatement,
      aux_sym_oneof_repeat1,
    ACTIONS(126), 15,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
  [768] = 12,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(27), 1,
      sym_ident,
    ACTIONS(29), 1,
      anon_sym_DOT,
    ACTIONS(94), 1,
      anon_sym_SEMI,
    ACTIONS(96), 1,
      anon_sym_option,
    ACTIONS(135), 1,
      anon_sym_RBRACE,
    STATE(130), 1,
      sym_messageName,
    STATE(142), 1,
      aux_sym_enumMessageType_repeat1,
    STATE(144), 1,
      sym_enumMessageType,
    STATE(177), 1,
      sym_type,
    STATE(11), 4,
      sym_option,
      sym_oneofField,
      sym_emptyStatement,
      aux_sym_oneof_repeat1,
    ACTIONS(23), 15,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
  [822] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(92), 1,
      anon_sym_COMMA,
    STATE(12), 1,
      aux_sym_ranges_repeat1,
    ACTIONS(137), 3,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_DOT,
    ACTIONS(139), 23,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_repeated,
      anon_sym_oneof,
      anon_sym_map,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_reserved,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [862] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(141), 4,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_COMMA,
      anon_sym_DOT,
    ACTIONS(143), 23,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_repeated,
      anon_sym_oneof,
      anon_sym_map,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_reserved,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [897] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(100), 4,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_COMMA,
      anon_sym_DOT,
    ACTIONS(102), 23,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_repeated,
      anon_sym_oneof,
      anon_sym_map,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_reserved,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [932] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(111), 4,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_COMMA,
      anon_sym_DOT,
    ACTIONS(113), 23,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_repeated,
      anon_sym_oneof,
      anon_sym_map,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_reserved,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [967] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(145), 3,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_DOT,
    ACTIONS(147), 23,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_repeated,
      anon_sym_oneof,
      anon_sym_map,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_reserved,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [1001] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(149), 3,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_DOT,
    ACTIONS(151), 23,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_repeated,
      anon_sym_oneof,
      anon_sym_map,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_reserved,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [1035] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(153), 3,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_DOT,
    ACTIONS(155), 23,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_repeated,
      anon_sym_oneof,
      anon_sym_map,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_reserved,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [1069] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(157), 3,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_DOT,
    ACTIONS(159), 23,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_repeated,
      anon_sym_oneof,
      anon_sym_map,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_reserved,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [1103] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(161), 3,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_DOT,
    ACTIONS(163), 23,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_repeated,
      anon_sym_oneof,
      anon_sym_map,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_reserved,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [1137] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(165), 3,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_DOT,
    ACTIONS(167), 23,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_repeated,
      anon_sym_oneof,
      anon_sym_map,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_reserved,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [1171] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(169), 3,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_DOT,
    ACTIONS(171), 23,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_repeated,
      anon_sym_oneof,
      anon_sym_map,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_reserved,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [1205] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(173), 3,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_DOT,
    ACTIONS(175), 23,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_repeated,
      anon_sym_oneof,
      anon_sym_map,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_reserved,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [1239] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(177), 3,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_DOT,
    ACTIONS(179), 23,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_repeated,
      anon_sym_oneof,
      anon_sym_map,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_reserved,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [1273] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(181), 3,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_DOT,
    ACTIONS(183), 23,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_repeated,
      anon_sym_oneof,
      anon_sym_map,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_reserved,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [1307] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(185), 3,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_DOT,
    ACTIONS(187), 23,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_repeated,
      anon_sym_oneof,
      anon_sym_map,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_reserved,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [1341] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(189), 3,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_DOT,
    ACTIONS(191), 23,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_repeated,
      anon_sym_oneof,
      anon_sym_map,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_reserved,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [1375] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(193), 3,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_DOT,
    ACTIONS(195), 23,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_repeated,
      anon_sym_oneof,
      anon_sym_map,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_reserved,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [1409] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(197), 3,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_DOT,
    ACTIONS(199), 23,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_repeated,
      anon_sym_oneof,
      anon_sym_map,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_reserved,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [1443] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(201), 3,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_DOT,
    ACTIONS(203), 23,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_repeated,
      anon_sym_oneof,
      anon_sym_map,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_reserved,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [1477] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(205), 3,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_DOT,
    ACTIONS(207), 23,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_repeated,
      anon_sym_oneof,
      anon_sym_map,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_reserved,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [1511] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(209), 3,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_DOT,
    ACTIONS(211), 23,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_repeated,
      anon_sym_oneof,
      anon_sym_map,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_reserved,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [1545] = 8,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(27), 1,
      sym_ident,
    ACTIONS(29), 1,
      anon_sym_DOT,
    STATE(130), 1,
      sym_messageName,
    STATE(142), 1,
      aux_sym_enumMessageType_repeat1,
    STATE(144), 1,
      sym_enumMessageType,
    STATE(241), 1,
      sym_type,
    ACTIONS(23), 15,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
  [1584] = 8,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(27), 1,
      sym_ident,
    ACTIONS(29), 1,
      anon_sym_DOT,
    STATE(130), 1,
      sym_messageName,
    STATE(142), 1,
      aux_sym_enumMessageType_repeat1,
    STATE(144), 1,
      sym_enumMessageType,
    STATE(185), 1,
      sym_type,
    ACTIONS(23), 15,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
  [1623] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(213), 3,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_DOT,
    ACTIONS(215), 17,
      anon_sym_option,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [1651] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(201), 3,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_DOT,
    ACTIONS(203), 17,
      anon_sym_option,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [1679] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(217), 3,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_DOT,
    ACTIONS(219), 17,
      anon_sym_option,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [1707] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(197), 3,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_DOT,
    ACTIONS(199), 17,
      anon_sym_option,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
      anon_sym_double,
      anon_sym_float,
      anon_sym_bytes,
      sym_ident,
  [1735] = 11,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(221), 1,
      anon_sym_LBRACE,
    ACTIONS(229), 1,
      anon_sym_DQUOTE,
    ACTIONS(231), 1,
      anon_sym_SQUOTE,
    ACTIONS(233), 1,
      aux_sym_constant_token1,
    ACTIONS(235), 1,
      sym_ident,
    STATE(193), 1,
      sym_constant,
    ACTIONS(223), 2,
      sym_intLit,
      sym_boolLit,
    ACTIONS(227), 2,
      anon_sym_inf,
      anon_sym_nan,
    ACTIONS(225), 3,
      aux_sym_floatLit_token1,
      aux_sym_floatLit_token2,
      aux_sym_floatLit_token3,
    STATE(97), 4,
      sym_floatLit,
      sym_strLit,
      sym_msgLit,
      sym_fullIdent,
  [1776] = 11,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(221), 1,
      anon_sym_LBRACE,
    ACTIONS(229), 1,
      anon_sym_DQUOTE,
    ACTIONS(231), 1,
      anon_sym_SQUOTE,
    ACTIONS(233), 1,
      aux_sym_constant_token1,
    ACTIONS(235), 1,
      sym_ident,
    STATE(240), 1,
      sym_constant,
    ACTIONS(223), 2,
      sym_intLit,
      sym_boolLit,
    ACTIONS(227), 2,
      anon_sym_inf,
      anon_sym_nan,
    ACTIONS(225), 3,
      aux_sym_floatLit_token1,
      aux_sym_floatLit_token2,
      aux_sym_floatLit_token3,
    STATE(97), 4,
      sym_floatLit,
      sym_strLit,
      sym_msgLit,
      sym_fullIdent,
  [1817] = 11,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(221), 1,
      anon_sym_LBRACE,
    ACTIONS(229), 1,
      anon_sym_DQUOTE,
    ACTIONS(231), 1,
      anon_sym_SQUOTE,
    ACTIONS(233), 1,
      aux_sym_constant_token1,
    ACTIONS(235), 1,
      sym_ident,
    STATE(167), 1,
      sym_constant,
    ACTIONS(223), 2,
      sym_intLit,
      sym_boolLit,
    ACTIONS(227), 2,
      anon_sym_inf,
      anon_sym_nan,
    ACTIONS(225), 3,
      aux_sym_floatLit_token1,
      aux_sym_floatLit_token2,
      aux_sym_floatLit_token3,
    STATE(97), 4,
      sym_floatLit,
      sym_strLit,
      sym_msgLit,
      sym_fullIdent,
  [1858] = 11,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(221), 1,
      anon_sym_LBRACE,
    ACTIONS(229), 1,
      anon_sym_DQUOTE,
    ACTIONS(231), 1,
      anon_sym_SQUOTE,
    ACTIONS(233), 1,
      aux_sym_constant_token1,
    ACTIONS(235), 1,
      sym_ident,
    STATE(182), 1,
      sym_constant,
    ACTIONS(223), 2,
      sym_intLit,
      sym_boolLit,
    ACTIONS(227), 2,
      anon_sym_inf,
      anon_sym_nan,
    ACTIONS(225), 3,
      aux_sym_floatLit_token1,
      aux_sym_floatLit_token2,
      aux_sym_floatLit_token3,
    STATE(97), 4,
      sym_floatLit,
      sym_strLit,
      sym_msgLit,
      sym_fullIdent,
  [1899] = 11,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(221), 1,
      anon_sym_LBRACE,
    ACTIONS(229), 1,
      anon_sym_DQUOTE,
    ACTIONS(231), 1,
      anon_sym_SQUOTE,
    ACTIONS(233), 1,
      aux_sym_constant_token1,
    ACTIONS(235), 1,
      sym_ident,
    STATE(236), 1,
      sym_constant,
    ACTIONS(223), 2,
      sym_intLit,
      sym_boolLit,
    ACTIONS(227), 2,
      anon_sym_inf,
      anon_sym_nan,
    ACTIONS(225), 3,
      aux_sym_floatLit_token1,
      aux_sym_floatLit_token2,
      aux_sym_floatLit_token3,
    STATE(97), 4,
      sym_floatLit,
      sym_strLit,
      sym_msgLit,
      sym_fullIdent,
  [1940] = 11,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(221), 1,
      anon_sym_LBRACE,
    ACTIONS(229), 1,
      anon_sym_DQUOTE,
    ACTIONS(231), 1,
      anon_sym_SQUOTE,
    ACTIONS(233), 1,
      aux_sym_constant_token1,
    ACTIONS(235), 1,
      sym_ident,
    STATE(184), 1,
      sym_constant,
    ACTIONS(223), 2,
      sym_intLit,
      sym_boolLit,
    ACTIONS(227), 2,
      anon_sym_inf,
      anon_sym_nan,
    ACTIONS(225), 3,
      aux_sym_floatLit_token1,
      aux_sym_floatLit_token2,
      aux_sym_floatLit_token3,
    STATE(97), 4,
      sym_floatLit,
      sym_strLit,
      sym_msgLit,
      sym_fullIdent,
  [1981] = 11,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(221), 1,
      anon_sym_LBRACE,
    ACTIONS(229), 1,
      anon_sym_DQUOTE,
    ACTIONS(231), 1,
      anon_sym_SQUOTE,
    ACTIONS(233), 1,
      aux_sym_constant_token1,
    ACTIONS(235), 1,
      sym_ident,
    STATE(239), 1,
      sym_constant,
    ACTIONS(223), 2,
      sym_intLit,
      sym_boolLit,
    ACTIONS(227), 2,
      anon_sym_inf,
      anon_sym_nan,
    ACTIONS(225), 3,
      aux_sym_floatLit_token1,
      aux_sym_floatLit_token2,
      aux_sym_floatLit_token3,
    STATE(97), 4,
      sym_floatLit,
      sym_strLit,
      sym_msgLit,
      sym_fullIdent,
  [2022] = 10,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(237), 1,
      ts_builtin_sym_end,
    ACTIONS(239), 1,
      anon_sym_SEMI,
    ACTIONS(241), 1,
      anon_sym_package,
    ACTIONS(243), 1,
      anon_sym_import,
    ACTIONS(245), 1,
      anon_sym_option,
    ACTIONS(247), 1,
      anon_sym_enum,
    ACTIONS(249), 1,
      anon_sym_message,
    ACTIONS(251), 1,
      anon_sym_service,
    STATE(53), 8,
      sym_package,
      sym_import,
      sym_option,
      sym_enum,
      sym_message,
      sym_service,
      sym_emptyStatement,
      aux_sym_source_file_repeat1,
  [2060] = 10,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(239), 1,
      anon_sym_SEMI,
    ACTIONS(241), 1,
      anon_sym_package,
    ACTIONS(243), 1,
      anon_sym_import,
    ACTIONS(245), 1,
      anon_sym_option,
    ACTIONS(247), 1,
      anon_sym_enum,
    ACTIONS(249), 1,
      anon_sym_message,
    ACTIONS(251), 1,
      anon_sym_service,
    ACTIONS(253), 1,
      ts_builtin_sym_end,
    STATE(51), 8,
      sym_package,
      sym_import,
      sym_option,
      sym_enum,
      sym_message,
      sym_service,
      sym_emptyStatement,
      aux_sym_source_file_repeat1,
  [2098] = 10,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(255), 1,
      ts_builtin_sym_end,
    ACTIONS(257), 1,
      anon_sym_SEMI,
    ACTIONS(260), 1,
      anon_sym_package,
    ACTIONS(263), 1,
      anon_sym_import,
    ACTIONS(266), 1,
      anon_sym_option,
    ACTIONS(269), 1,
      anon_sym_enum,
    ACTIONS(272), 1,
      anon_sym_message,
    ACTIONS(275), 1,
      anon_sym_service,
    STATE(53), 8,
      sym_package,
      sym_import,
      sym_option,
      sym_enum,
      sym_message,
      sym_service,
      sym_emptyStatement,
      aux_sym_source_file_repeat1,
  [2136] = 3,
    ACTIONS(3), 1,
      sym_comment,
    STATE(217), 1,
      sym_keyType,
    ACTIONS(278), 12,
      anon_sym_int32,
      anon_sym_int64,
      anon_sym_uint32,
      anon_sym_uint64,
      anon_sym_sint32,
      anon_sym_sint64,
      anon_sym_fixed32,
      anon_sym_fixed64,
      anon_sym_sfixed32,
      anon_sym_sfixed64,
      anon_sym_bool,
      anon_sym_string,
  [2157] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(201), 10,
      ts_builtin_sym_end,
      anon_sym_SEMI,
      anon_sym_package,
      anon_sym_import,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_RBRACE,
      anon_sym_message,
      anon_sym_service,
      anon_sym_rpc,
  [2173] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(197), 10,
      ts_builtin_sym_end,
      anon_sym_SEMI,
      anon_sym_package,
      anon_sym_import,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_RBRACE,
      anon_sym_message,
      anon_sym_service,
      anon_sym_rpc,
  [2189] = 4,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(282), 1,
      anon_sym_DOT,
    STATE(57), 1,
      aux_sym_fullIdent_repeat1,
    ACTIONS(280), 7,
      anon_sym_EQ,
      anon_sym_SEMI,
      anon_sym_RPAREN,
      anon_sym_RBRACE,
      anon_sym_COMMA,
      anon_sym_RBRACK,
      sym_ident,
  [2208] = 4,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(287), 1,
      anon_sym_DOT,
    STATE(57), 1,
      aux_sym_fullIdent_repeat1,
    ACTIONS(285), 7,
      anon_sym_EQ,
      anon_sym_SEMI,
      anon_sym_RPAREN,
      anon_sym_RBRACE,
      anon_sym_COMMA,
      anon_sym_RBRACK,
      sym_ident,
  [2227] = 4,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(287), 1,
      anon_sym_DOT,
    STATE(58), 1,
      aux_sym_fullIdent_repeat1,
    ACTIONS(289), 7,
      anon_sym_EQ,
      anon_sym_SEMI,
      anon_sym_RPAREN,
      anon_sym_RBRACE,
      anon_sym_COMMA,
      anon_sym_RBRACK,
      sym_ident,
  [2246] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(280), 8,
      anon_sym_EQ,
      anon_sym_SEMI,
      anon_sym_RPAREN,
      anon_sym_RBRACE,
      anon_sym_COMMA,
      anon_sym_RBRACK,
      sym_ident,
      anon_sym_DOT,
  [2260] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(185), 8,
      ts_builtin_sym_end,
      anon_sym_SEMI,
      anon_sym_package,
      anon_sym_import,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_service,
  [2274] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(209), 8,
      ts_builtin_sym_end,
      anon_sym_SEMI,
      anon_sym_package,
      anon_sym_import,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_service,
  [2288] = 6,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(291), 1,
      anon_sym_SEMI,
    ACTIONS(294), 1,
      anon_sym_option,
    ACTIONS(297), 1,
      anon_sym_RBRACE,
    ACTIONS(299), 1,
      anon_sym_rpc,
    STATE(63), 4,
      sym_option,
      sym_rpc,
      sym_emptyStatement,
      aux_sym_service_repeat1,
  [2310] = 6,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(302), 1,
      anon_sym_SEMI,
    ACTIONS(304), 1,
      anon_sym_option,
    ACTIONS(306), 1,
      anon_sym_RBRACE,
    ACTIONS(308), 1,
      sym_ident,
    STATE(79), 4,
      sym_option,
      sym_enumField,
      sym_emptyStatement,
      aux_sym_enumBody_repeat1,
  [2332] = 6,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(239), 1,
      anon_sym_SEMI,
    ACTIONS(245), 1,
      anon_sym_option,
    ACTIONS(310), 1,
      anon_sym_RBRACE,
    ACTIONS(312), 1,
      anon_sym_rpc,
    STATE(70), 4,
      sym_option,
      sym_rpc,
      sym_emptyStatement,
      aux_sym_service_repeat1,
  [2354] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(153), 8,
      ts_builtin_sym_end,
      anon_sym_SEMI,
      anon_sym_package,
      anon_sym_import,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_service,
  [2368] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(157), 8,
      ts_builtin_sym_end,
      anon_sym_SEMI,
      anon_sym_package,
      anon_sym_import,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_service,
  [2382] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(314), 8,
      ts_builtin_sym_end,
      anon_sym_SEMI,
      anon_sym_package,
      anon_sym_import,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_service,
  [2396] = 6,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(302), 1,
      anon_sym_SEMI,
    ACTIONS(304), 1,
      anon_sym_option,
    ACTIONS(308), 1,
      sym_ident,
    ACTIONS(316), 1,
      anon_sym_RBRACE,
    STATE(74), 4,
      sym_option,
      sym_enumField,
      sym_emptyStatement,
      aux_sym_enumBody_repeat1,
  [2418] = 6,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(239), 1,
      anon_sym_SEMI,
    ACTIONS(245), 1,
      anon_sym_option,
    ACTIONS(312), 1,
      anon_sym_rpc,
    ACTIONS(318), 1,
      anon_sym_RBRACE,
    STATE(63), 4,
      sym_option,
      sym_rpc,
      sym_emptyStatement,
      aux_sym_service_repeat1,
  [2440] = 6,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(302), 1,
      anon_sym_SEMI,
    ACTIONS(304), 1,
      anon_sym_option,
    ACTIONS(308), 1,
      sym_ident,
    ACTIONS(320), 1,
      anon_sym_RBRACE,
    STATE(64), 4,
      sym_option,
      sym_enumField,
      sym_emptyStatement,
      aux_sym_enumBody_repeat1,
  [2462] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(322), 8,
      ts_builtin_sym_end,
      anon_sym_SEMI,
      anon_sym_package,
      anon_sym_import,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_service,
  [2476] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(193), 8,
      ts_builtin_sym_end,
      anon_sym_SEMI,
      anon_sym_package,
      anon_sym_import,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_service,
  [2490] = 6,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(302), 1,
      anon_sym_SEMI,
    ACTIONS(304), 1,
      anon_sym_option,
    ACTIONS(308), 1,
      sym_ident,
    ACTIONS(324), 1,
      anon_sym_RBRACE,
    STATE(79), 4,
      sym_option,
      sym_enumField,
      sym_emptyStatement,
      aux_sym_enumBody_repeat1,
  [2512] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(326), 8,
      ts_builtin_sym_end,
      anon_sym_SEMI,
      anon_sym_package,
      anon_sym_import,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_service,
  [2526] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(328), 8,
      ts_builtin_sym_end,
      anon_sym_SEMI,
      anon_sym_package,
      anon_sym_import,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_service,
  [2540] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(330), 8,
      ts_builtin_sym_end,
      anon_sym_SEMI,
      anon_sym_package,
      anon_sym_import,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_service,
  [2554] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(205), 8,
      ts_builtin_sym_end,
      anon_sym_SEMI,
      anon_sym_package,
      anon_sym_import,
      anon_sym_option,
      anon_sym_enum,
      anon_sym_message,
      anon_sym_service,
  [2568] = 6,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(332), 1,
      anon_sym_SEMI,
    ACTIONS(335), 1,
      anon_sym_option,
    ACTIONS(338), 1,
      anon_sym_RBRACE,
    ACTIONS(340), 1,
      sym_ident,
    STATE(79), 4,
      sym_option,
      sym_enumField,
      sym_emptyStatement,
      aux_sym_enumBody_repeat1,
  [2590] = 4,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(343), 1,
      sym_intLit,
    STATE(102), 1,
      sym_floatLit,
    ACTIONS(225), 5,
      aux_sym_floatLit_token1,
      aux_sym_floatLit_token2,
      aux_sym_floatLit_token3,
      anon_sym_inf,
      anon_sym_nan,
  [2607] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(239), 1,
      anon_sym_SEMI,
    ACTIONS(245), 1,
      anon_sym_option,
    ACTIONS(345), 1,
      anon_sym_RBRACE,
    STATE(92), 3,
      sym_option,
      sym_emptyStatement,
      aux_sym_rpc_repeat1,
  [2625] = 7,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(347), 1,
      anon_sym_LPAREN,
    ACTIONS(349), 1,
      sym_ident,
    STATE(137), 1,
      sym_fieldOption,
    STATE(215), 1,
      sym_fieldOptions,
    STATE(222), 1,
      sym_fullIdent,
    STATE(232), 1,
      sym_optionName,
  [2647] = 7,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(27), 1,
      sym_ident,
    ACTIONS(29), 1,
      anon_sym_DOT,
    ACTIONS(351), 1,
      anon_sym_stream,
    STATE(130), 1,
      sym_messageName,
    STATE(142), 1,
      aux_sym_enumMessageType_repeat1,
    STATE(195), 1,
      sym_enumMessageType,
  [2669] = 7,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(27), 1,
      sym_ident,
    ACTIONS(29), 1,
      anon_sym_DOT,
    ACTIONS(353), 1,
      anon_sym_stream,
    STATE(130), 1,
      sym_messageName,
    STATE(142), 1,
      aux_sym_enumMessageType_repeat1,
    STATE(186), 1,
      sym_enumMessageType,
  [2691] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(239), 1,
      anon_sym_SEMI,
    ACTIONS(245), 1,
      anon_sym_option,
    ACTIONS(355), 1,
      anon_sym_RBRACE,
    STATE(86), 3,
      sym_option,
      sym_emptyStatement,
      aux_sym_rpc_repeat1,
  [2709] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(239), 1,
      anon_sym_SEMI,
    ACTIONS(245), 1,
      anon_sym_option,
    ACTIONS(345), 1,
      anon_sym_RBRACE,
    STATE(95), 3,
      sym_option,
      sym_emptyStatement,
      aux_sym_rpc_repeat1,
  [2727] = 7,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(347), 1,
      anon_sym_LPAREN,
    ACTIONS(349), 1,
      sym_ident,
    STATE(137), 1,
      sym_fieldOption,
    STATE(206), 1,
      sym_fieldOptions,
    STATE(222), 1,
      sym_fullIdent,
    STATE(232), 1,
      sym_optionName,
  [2749] = 7,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(347), 1,
      anon_sym_LPAREN,
    ACTIONS(349), 1,
      sym_ident,
    STATE(137), 1,
      sym_fieldOption,
    STATE(222), 1,
      sym_fullIdent,
    STATE(231), 1,
      sym_fieldOptions,
    STATE(232), 1,
      sym_optionName,
  [2771] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(239), 1,
      anon_sym_SEMI,
    ACTIONS(245), 1,
      anon_sym_option,
    ACTIONS(357), 1,
      anon_sym_RBRACE,
    STATE(95), 3,
      sym_option,
      sym_emptyStatement,
      aux_sym_rpc_repeat1,
  [2789] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(239), 1,
      anon_sym_SEMI,
    ACTIONS(245), 1,
      anon_sym_option,
    ACTIONS(359), 1,
      anon_sym_RBRACE,
    STATE(89), 3,
      sym_option,
      sym_emptyStatement,
      aux_sym_rpc_repeat1,
  [2807] = 7,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(347), 1,
      anon_sym_LPAREN,
    ACTIONS(349), 1,
      sym_ident,
    STATE(137), 1,
      sym_fieldOption,
    STATE(196), 1,
      sym_fieldOptions,
    STATE(222), 1,
      sym_fullIdent,
    STATE(232), 1,
      sym_optionName,
  [2829] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(239), 1,
      anon_sym_SEMI,
    ACTIONS(245), 1,
      anon_sym_option,
    ACTIONS(359), 1,
      anon_sym_RBRACE,
    STATE(95), 3,
      sym_option,
      sym_emptyStatement,
      aux_sym_rpc_repeat1,
  [2847] = 6,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(361), 1,
      sym_intLit,
    ACTIONS(363), 1,
      sym_ident,
    STATE(9), 1,
      sym_fieldName,
    STATE(10), 1,
      sym_range,
    STATE(29), 2,
      sym_ranges,
      sym_fieldNames,
  [2867] = 7,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(27), 1,
      sym_ident,
    ACTIONS(29), 1,
      anon_sym_DOT,
    ACTIONS(365), 1,
      anon_sym_stream,
    STATE(130), 1,
      sym_messageName,
    STATE(142), 1,
      aux_sym_enumMessageType_repeat1,
    STATE(250), 1,
      sym_enumMessageType,
  [2889] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(367), 1,
      anon_sym_SEMI,
    ACTIONS(370), 1,
      anon_sym_option,
    ACTIONS(373), 1,
      anon_sym_RBRACE,
    STATE(95), 3,
      sym_option,
      sym_emptyStatement,
      aux_sym_rpc_repeat1,
  [2907] = 6,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(29), 1,
      anon_sym_DOT,
    ACTIONS(375), 1,
      sym_ident,
    STATE(130), 1,
      sym_messageName,
    STATE(142), 1,
      aux_sym_enumMessageType_repeat1,
    STATE(186), 1,
      sym_enumMessageType,
  [2926] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(377), 5,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_COMMA,
      anon_sym_RBRACK,
      sym_ident,
  [2937] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(379), 5,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_COMMA,
      anon_sym_RBRACK,
      sym_ident,
  [2948] = 6,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(347), 1,
      anon_sym_LPAREN,
    ACTIONS(349), 1,
      sym_ident,
    STATE(131), 1,
      sym_enumValueOption,
    STATE(222), 1,
      sym_fullIdent,
    STATE(243), 1,
      sym_optionName,
  [2967] = 6,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(347), 1,
      anon_sym_LPAREN,
    ACTIONS(349), 1,
      sym_ident,
    STATE(168), 1,
      sym_enumValueOption,
    STATE(222), 1,
      sym_fullIdent,
    STATE(243), 1,
      sym_optionName,
  [2986] = 6,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(29), 1,
      anon_sym_DOT,
    ACTIONS(375), 1,
      sym_ident,
    STATE(130), 1,
      sym_messageName,
    STATE(142), 1,
      aux_sym_enumMessageType_repeat1,
    STATE(235), 1,
      sym_enumMessageType,
  [3005] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(381), 5,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_COMMA,
      anon_sym_RBRACK,
      sym_ident,
  [3016] = 6,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(347), 1,
      anon_sym_LPAREN,
    ACTIONS(349), 1,
      sym_ident,
    STATE(145), 1,
      sym_fieldOption,
    STATE(222), 1,
      sym_fullIdent,
    STATE(232), 1,
      sym_optionName,
  [3035] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(383), 5,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_COMMA,
      anon_sym_RBRACK,
      sym_ident,
  [3046] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(385), 5,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_COMMA,
      anon_sym_RBRACK,
      sym_ident,
  [3057] = 6,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(347), 1,
      anon_sym_LPAREN,
    ACTIONS(349), 1,
      sym_ident,
    STATE(138), 1,
      sym_enumValueOption,
    STATE(222), 1,
      sym_fullIdent,
    STATE(243), 1,
      sym_optionName,
  [3076] = 6,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(29), 1,
      anon_sym_DOT,
    ACTIONS(375), 1,
      sym_ident,
    STATE(130), 1,
      sym_messageName,
    STATE(142), 1,
      aux_sym_enumMessageType_repeat1,
    STATE(201), 1,
      sym_enumMessageType,
  [3095] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(387), 5,
      anon_sym_SEMI,
      anon_sym_RBRACE,
      anon_sym_COMMA,
      anon_sym_RBRACK,
      sym_ident,
  [3106] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(201), 2,
      anon_sym_SEMI,
      anon_sym_RBRACE,
    ACTIONS(203), 2,
      anon_sym_option,
      sym_ident,
  [3118] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(347), 1,
      anon_sym_LPAREN,
    ACTIONS(349), 1,
      sym_ident,
    STATE(222), 1,
      sym_fullIdent,
    STATE(247), 1,
      sym_optionName,
  [3134] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(391), 1,
      anon_sym_DOT,
    ACTIONS(389), 3,
      anon_sym_RPAREN,
      anon_sym_GT,
      sym_ident,
  [3146] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(393), 4,
      anon_sym_SEMI,
      anon_sym_option,
      anon_sym_RBRACE,
      anon_sym_rpc,
  [3156] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(395), 4,
      anon_sym_SEMI,
      anon_sym_option,
      anon_sym_RBRACE,
      anon_sym_rpc,
  [3166] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(397), 4,
      anon_sym_SEMI,
      anon_sym_option,
      anon_sym_RBRACE,
      anon_sym_rpc,
  [3176] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(399), 4,
      anon_sym_SEMI,
      anon_sym_option,
      anon_sym_RBRACE,
      anon_sym_rpc,
  [3186] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(401), 4,
      anon_sym_SEMI,
      anon_sym_option,
      anon_sym_RBRACE,
      anon_sym_rpc,
  [3196] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(403), 2,
      anon_sym_SEMI,
      anon_sym_RBRACE,
    ACTIONS(405), 2,
      anon_sym_option,
      sym_ident,
  [3208] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(363), 1,
      sym_ident,
    ACTIONS(407), 1,
      anon_sym_RBRACE,
    STATE(128), 1,
      aux_sym_msgLit_repeat1,
    STATE(200), 1,
      sym_fieldName,
  [3224] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(409), 2,
      anon_sym_SEMI,
      anon_sym_RBRACE,
    ACTIONS(411), 2,
      anon_sym_option,
      sym_ident,
  [3236] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(413), 2,
      anon_sym_SEMI,
      anon_sym_RBRACE,
    ACTIONS(415), 2,
      anon_sym_option,
      sym_ident,
  [3248] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(347), 1,
      anon_sym_LPAREN,
    ACTIONS(349), 1,
      sym_ident,
    STATE(222), 1,
      sym_fullIdent,
    STATE(248), 1,
      sym_optionName,
  [3264] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(417), 1,
      anon_sym_RBRACE,
    ACTIONS(419), 1,
      sym_ident,
    STATE(122), 1,
      aux_sym_msgLit_repeat1,
    STATE(200), 1,
      sym_fieldName,
  [3280] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(197), 2,
      anon_sym_SEMI,
      anon_sym_RBRACE,
    ACTIONS(199), 2,
      anon_sym_option,
      sym_ident,
  [3292] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(422), 2,
      anon_sym_SEMI,
      anon_sym_RBRACE,
    ACTIONS(424), 2,
      anon_sym_option,
      sym_ident,
  [3304] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(347), 1,
      anon_sym_LPAREN,
    ACTIONS(349), 1,
      sym_ident,
    STATE(222), 1,
      sym_fullIdent,
    STATE(246), 1,
      sym_optionName,
  [3320] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(426), 2,
      anon_sym_SEMI,
      anon_sym_RBRACE,
    ACTIONS(428), 2,
      anon_sym_option,
      sym_ident,
  [3332] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(347), 1,
      anon_sym_LPAREN,
    ACTIONS(349), 1,
      sym_ident,
    STATE(222), 1,
      sym_fullIdent,
    STATE(223), 1,
      sym_optionName,
  [3348] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(363), 1,
      sym_ident,
    ACTIONS(430), 1,
      anon_sym_RBRACE,
    STATE(122), 1,
      aux_sym_msgLit_repeat1,
    STATE(200), 1,
      sym_fieldName,
  [3364] = 4,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(229), 1,
      anon_sym_DQUOTE,
    ACTIONS(231), 1,
      anon_sym_SQUOTE,
    STATE(226), 1,
      sym_strLit,
  [3377] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(432), 3,
      anon_sym_RPAREN,
      anon_sym_GT,
      sym_ident,
  [3386] = 4,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(434), 1,
      anon_sym_COMMA,
    ACTIONS(436), 1,
      anon_sym_RBRACK,
    STATE(132), 1,
      aux_sym_enumField_repeat1,
  [3399] = 4,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(434), 1,
      anon_sym_COMMA,
    ACTIONS(438), 1,
      anon_sym_RBRACK,
    STATE(143), 1,
      aux_sym_enumField_repeat1,
  [3412] = 4,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(440), 1,
      anon_sym_COMMA,
    ACTIONS(442), 1,
      anon_sym_RBRACK,
    STATE(134), 1,
      aux_sym_fieldOptions_repeat1,
  [3425] = 4,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(444), 1,
      anon_sym_COMMA,
    ACTIONS(447), 1,
      anon_sym_RBRACK,
    STATE(134), 1,
      aux_sym_fieldOptions_repeat1,
  [3438] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(449), 3,
      anon_sym_RPAREN,
      anon_sym_GT,
      sym_ident,
  [3447] = 4,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(375), 1,
      sym_ident,
    STATE(135), 1,
      sym_messageName,
    STATE(147), 1,
      aux_sym_enumMessageType_repeat1,
  [3460] = 4,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(440), 1,
      anon_sym_COMMA,
    ACTIONS(451), 1,
      anon_sym_RBRACK,
    STATE(133), 1,
      aux_sym_fieldOptions_repeat1,
  [3473] = 4,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(434), 1,
      anon_sym_COMMA,
    ACTIONS(453), 1,
      anon_sym_RBRACK,
    STATE(139), 1,
      aux_sym_enumField_repeat1,
  [3486] = 4,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(434), 1,
      anon_sym_COMMA,
    ACTIONS(436), 1,
      anon_sym_RBRACK,
    STATE(143), 1,
      aux_sym_enumField_repeat1,
  [3499] = 4,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(375), 1,
      sym_ident,
    STATE(136), 1,
      aux_sym_enumMessageType_repeat1,
    STATE(141), 1,
      sym_messageName,
  [3512] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(455), 3,
      anon_sym_RPAREN,
      anon_sym_GT,
      sym_ident,
  [3521] = 4,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(375), 1,
      sym_ident,
    STATE(141), 1,
      sym_messageName,
    STATE(147), 1,
      aux_sym_enumMessageType_repeat1,
  [3534] = 4,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(457), 1,
      anon_sym_COMMA,
    ACTIONS(460), 1,
      anon_sym_RBRACK,
    STATE(143), 1,
      aux_sym_enumField_repeat1,
  [3547] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(462), 2,
      anon_sym_GT,
      sym_ident,
  [3555] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(447), 2,
      anon_sym_COMMA,
      anon_sym_RBRACK,
  [3563] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(464), 1,
      anon_sym_DASH,
    ACTIONS(466), 1,
      sym_intLit,
  [3573] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(468), 1,
      sym_ident,
    STATE(147), 1,
      aux_sym_enumMessageType_repeat1,
  [3583] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(471), 2,
      anon_sym_SEMI,
      anon_sym_LBRACK,
  [3591] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(473), 1,
      anon_sym_SEMI,
    ACTIONS(475), 1,
      anon_sym_LBRACK,
  [3601] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(349), 1,
      sym_ident,
    STATE(229), 1,
      sym_fullIdent,
  [3611] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(477), 1,
      sym_ident,
    STATE(157), 1,
      sym_messageName,
  [3621] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(479), 1,
      anon_sym_SEMI,
    ACTIONS(481), 1,
      anon_sym_LBRACK,
  [3631] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(483), 1,
      sym_ident,
    STATE(159), 1,
      sym_enumName,
  [3641] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(485), 1,
      sym_ident,
    STATE(238), 1,
      sym_rpcName,
  [3651] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(483), 1,
      sym_ident,
    STATE(165), 1,
      sym_enumName,
  [3661] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(477), 1,
      sym_ident,
    STATE(166), 1,
      sym_messageName,
  [3671] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(487), 1,
      anon_sym_LBRACE,
    STATE(23), 1,
      sym_messageBody,
  [3681] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(363), 1,
      sym_ident,
    STATE(251), 1,
      sym_fieldName,
  [3691] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(489), 1,
      anon_sym_LBRACE,
    STATE(24), 1,
      sym_enumBody,
  [3701] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(491), 1,
      sym_intLit,
    STATE(170), 1,
      sym_fieldNumber,
  [3711] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(493), 1,
      sym_ident,
    STATE(208), 1,
      sym_mapName,
  [3721] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(495), 1,
      sym_ident,
    STATE(213), 1,
      sym_serviceName,
  [3731] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(497), 1,
      anon_sym_SEMI,
    ACTIONS(499), 1,
      anon_sym_LBRACK,
  [3741] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(349), 1,
      sym_ident,
    STATE(204), 1,
      sym_fullIdent,
  [3751] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(501), 1,
      anon_sym_LBRACE,
    STATE(67), 1,
      sym_enumBody,
  [3761] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(503), 1,
      anon_sym_LBRACE,
    STATE(66), 1,
      sym_messageBody,
  [3771] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(505), 2,
      anon_sym_COMMA,
      anon_sym_RBRACK,
  [3779] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(460), 2,
      anon_sym_COMMA,
      anon_sym_RBRACK,
  [3787] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(507), 2,
      anon_sym_max,
      sym_intLit,
  [3795] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(509), 1,
      anon_sym_SEMI,
    ACTIONS(511), 1,
      anon_sym_LBRACK,
  [3805] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(361), 1,
      sym_intLit,
    STATE(19), 1,
      sym_range,
  [3815] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(345), 1,
      anon_sym_SEMI,
    ACTIONS(513), 1,
      anon_sym_LBRACE,
  [3825] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(363), 1,
      sym_ident,
    STATE(20), 1,
      sym_fieldName,
  [3835] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(355), 1,
      anon_sym_SEMI,
    ACTIONS(515), 1,
      anon_sym_LBRACE,
  [3845] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(491), 1,
      sym_intLit,
    STATE(149), 1,
      sym_fieldNumber,
  [3855] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(517), 1,
      anon_sym_SEMI,
    ACTIONS(519), 1,
      anon_sym_LBRACK,
  [3865] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(363), 1,
      sym_ident,
    STATE(242), 1,
      sym_fieldName,
  [3875] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(491), 1,
      sym_intLit,
    STATE(152), 1,
      sym_fieldNumber,
  [3885] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(521), 1,
      anon_sym_SEMI,
    ACTIONS(523), 1,
      anon_sym_LBRACE,
  [3895] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(525), 1,
      anon_sym_SEMI,
    ACTIONS(527), 1,
      anon_sym_LBRACK,
  [3905] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(529), 1,
      sym_ident,
    STATE(207), 1,
      sym_oneofName,
  [3915] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(531), 2,
      anon_sym_RBRACE,
      sym_ident,
  [3923] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(491), 1,
      sym_intLit,
    STATE(176), 1,
      sym_fieldNumber,
  [3933] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(533), 2,
      anon_sym_COMMA,
      anon_sym_RBRACK,
  [3941] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(363), 1,
      sym_ident,
    STATE(220), 1,
      sym_fieldName,
  [3951] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(535), 1,
      anon_sym_RPAREN,
  [3958] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(537), 1,
      anon_sym_COMMA,
  [3965] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(539), 1,
      anon_sym_LT,
  [3972] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(541), 1,
      anon_sym_LBRACE,
  [3979] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(543), 1,
      anon_sym_SEMI,
  [3986] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(545), 1,
      anon_sym_SEMI,
  [3993] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(547), 1,
      anon_sym_EQ,
  [4000] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(549), 1,
      anon_sym_SEMI,
  [4007] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(551), 1,
      anon_sym_EQ,
  [4014] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(553), 1,
      anon_sym_RPAREN,
  [4021] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(555), 1,
      anon_sym_RBRACK,
  [4028] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(391), 1,
      anon_sym_DOT,
  [4035] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(557), 1,
      anon_sym_SEMI,
  [4042] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(559), 1,
      anon_sym_EQ,
  [4049] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(561), 1,
      anon_sym_COLON,
  [4056] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(563), 1,
      anon_sym_RPAREN,
  [4063] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(565), 1,
      sym_ident,
  [4070] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(567), 1,
      anon_sym_SEMI,
  [4077] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(569), 1,
      anon_sym_RPAREN,
  [4084] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(571), 1,
      anon_sym_SQUOTE,
  [4091] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(573), 1,
      anon_sym_RBRACK,
  [4098] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(575), 1,
      anon_sym_LBRACE,
  [4105] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(577), 1,
      anon_sym_EQ,
  [4112] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(571), 1,
      anon_sym_DQUOTE,
  [4119] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(579), 1,
      sym_ident,
  [4126] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(581), 1,
      anon_sym_SEMI,
  [4133] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(583), 1,
      anon_sym_EQ,
  [4140] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(585), 1,
      anon_sym_LBRACE,
  [4147] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(587), 1,
      anon_sym_LBRACE,
  [4154] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(589), 1,
      anon_sym_RBRACK,
  [4161] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(389), 1,
      anon_sym_LBRACE,
  [4168] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(591), 1,
      anon_sym_COMMA,
  [4175] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(593), 1,
      anon_sym_SEMI,
  [4182] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(595), 1,
      anon_sym_LPAREN,
  [4189] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(597), 1,
      anon_sym_EQ,
  [4196] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(599), 1,
      anon_sym_LBRACE,
  [4203] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(601), 1,
      anon_sym_EQ,
  [4210] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(603), 1,
      anon_sym_EQ,
  [4217] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(605), 1,
      anon_sym_LPAREN,
  [4224] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(607), 1,
      anon_sym_returns,
  [4231] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(609), 1,
      anon_sym_SEMI,
  [4238] = 2,
    ACTIONS(611), 1,
      sym_comment,
    ACTIONS(613), 1,
      aux_sym_strLit_token2,
  [4245] = 2,
    ACTIONS(611), 1,
      sym_comment,
    ACTIONS(615), 1,
      aux_sym_strLit_token1,
  [4252] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(617), 1,
      anon_sym_SEMI,
  [4259] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(619), 1,
      anon_sym_SEMI,
  [4266] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(621), 1,
      anon_sym_RBRACK,
  [4273] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(623), 1,
      anon_sym_EQ,
  [4280] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(625), 1,
      anon_sym_SEMI,
  [4287] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(627), 1,
      anon_sym_returns,
  [4294] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(629), 1,
      anon_sym_RPAREN,
  [4301] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(631), 1,
      anon_sym_SEMI,
  [4308] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(633), 1,
      sym_intLit,
  [4315] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(635), 1,
      anon_sym_LPAREN,
  [4322] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(637), 1,
      anon_sym_SEMI,
  [4329] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(639), 1,
      anon_sym_SEMI,
  [4336] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(641), 1,
      anon_sym_GT,
  [4343] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(643), 1,
      anon_sym_EQ,
  [4350] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(645), 1,
      anon_sym_EQ,
  [4357] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(647), 1,
      aux_sym_syntax_token1,
  [4364] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(649), 1,
      ts_builtin_sym_end,
  [4371] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(651), 1,
      anon_sym_EQ,
  [4378] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(653), 1,
      anon_sym_EQ,
  [4385] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(655), 1,
      anon_sym_EQ,
  [4392] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(657), 1,
      anon_sym_LPAREN,
  [4399] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(659), 1,
      anon_sym_RPAREN,
  [4406] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(661), 1,
      anon_sym_EQ,
};

static const uint32_t ts_small_parse_table_map[] = {
  [SMALL_STATE(2)] = 0,
  [SMALL_STATE(3)] = 77,
  [SMALL_STATE(4)] = 154,
  [SMALL_STATE(5)] = 231,
  [SMALL_STATE(6)] = 308,
  [SMALL_STATE(7)] = 385,
  [SMALL_STATE(8)] = 422,
  [SMALL_STATE(9)] = 460,
  [SMALL_STATE(10)] = 500,
  [SMALL_STATE(11)] = 540,
  [SMALL_STATE(12)] = 594,
  [SMALL_STATE(13)] = 634,
  [SMALL_STATE(14)] = 674,
  [SMALL_STATE(15)] = 714,
  [SMALL_STATE(16)] = 768,
  [SMALL_STATE(17)] = 822,
  [SMALL_STATE(18)] = 862,
  [SMALL_STATE(19)] = 897,
  [SMALL_STATE(20)] = 932,
  [SMALL_STATE(21)] = 967,
  [SMALL_STATE(22)] = 1001,
  [SMALL_STATE(23)] = 1035,
  [SMALL_STATE(24)] = 1069,
  [SMALL_STATE(25)] = 1103,
  [SMALL_STATE(26)] = 1137,
  [SMALL_STATE(27)] = 1171,
  [SMALL_STATE(28)] = 1205,
  [SMALL_STATE(29)] = 1239,
  [SMALL_STATE(30)] = 1273,
  [SMALL_STATE(31)] = 1307,
  [SMALL_STATE(32)] = 1341,
  [SMALL_STATE(33)] = 1375,
  [SMALL_STATE(34)] = 1409,
  [SMALL_STATE(35)] = 1443,
  [SMALL_STATE(36)] = 1477,
  [SMALL_STATE(37)] = 1511,
  [SMALL_STATE(38)] = 1545,
  [SMALL_STATE(39)] = 1584,
  [SMALL_STATE(40)] = 1623,
  [SMALL_STATE(41)] = 1651,
  [SMALL_STATE(42)] = 1679,
  [SMALL_STATE(43)] = 1707,
  [SMALL_STATE(44)] = 1735,
  [SMALL_STATE(45)] = 1776,
  [SMALL_STATE(46)] = 1817,
  [SMALL_STATE(47)] = 1858,
  [SMALL_STATE(48)] = 1899,
  [SMALL_STATE(49)] = 1940,
  [SMALL_STATE(50)] = 1981,
  [SMALL_STATE(51)] = 2022,
  [SMALL_STATE(52)] = 2060,
  [SMALL_STATE(53)] = 2098,
  [SMALL_STATE(54)] = 2136,
  [SMALL_STATE(55)] = 2157,
  [SMALL_STATE(56)] = 2173,
  [SMALL_STATE(57)] = 2189,
  [SMALL_STATE(58)] = 2208,
  [SMALL_STATE(59)] = 2227,
  [SMALL_STATE(60)] = 2246,
  [SMALL_STATE(61)] = 2260,
  [SMALL_STATE(62)] = 2274,
  [SMALL_STATE(63)] = 2288,
  [SMALL_STATE(64)] = 2310,
  [SMALL_STATE(65)] = 2332,
  [SMALL_STATE(66)] = 2354,
  [SMALL_STATE(67)] = 2368,
  [SMALL_STATE(68)] = 2382,
  [SMALL_STATE(69)] = 2396,
  [SMALL_STATE(70)] = 2418,
  [SMALL_STATE(71)] = 2440,
  [SMALL_STATE(72)] = 2462,
  [SMALL_STATE(73)] = 2476,
  [SMALL_STATE(74)] = 2490,
  [SMALL_STATE(75)] = 2512,
  [SMALL_STATE(76)] = 2526,
  [SMALL_STATE(77)] = 2540,
  [SMALL_STATE(78)] = 2554,
  [SMALL_STATE(79)] = 2568,
  [SMALL_STATE(80)] = 2590,
  [SMALL_STATE(81)] = 2607,
  [SMALL_STATE(82)] = 2625,
  [SMALL_STATE(83)] = 2647,
  [SMALL_STATE(84)] = 2669,
  [SMALL_STATE(85)] = 2691,
  [SMALL_STATE(86)] = 2709,
  [SMALL_STATE(87)] = 2727,
  [SMALL_STATE(88)] = 2749,
  [SMALL_STATE(89)] = 2771,
  [SMALL_STATE(90)] = 2789,
  [SMALL_STATE(91)] = 2807,
  [SMALL_STATE(92)] = 2829,
  [SMALL_STATE(93)] = 2847,
  [SMALL_STATE(94)] = 2867,
  [SMALL_STATE(95)] = 2889,
  [SMALL_STATE(96)] = 2907,
  [SMALL_STATE(97)] = 2926,
  [SMALL_STATE(98)] = 2937,
  [SMALL_STATE(99)] = 2948,
  [SMALL_STATE(100)] = 2967,
  [SMALL_STATE(101)] = 2986,
  [SMALL_STATE(102)] = 3005,
  [SMALL_STATE(103)] = 3016,
  [SMALL_STATE(104)] = 3035,
  [SMALL_STATE(105)] = 3046,
  [SMALL_STATE(106)] = 3057,
  [SMALL_STATE(107)] = 3076,
  [SMALL_STATE(108)] = 3095,
  [SMALL_STATE(109)] = 3106,
  [SMALL_STATE(110)] = 3118,
  [SMALL_STATE(111)] = 3134,
  [SMALL_STATE(112)] = 3146,
  [SMALL_STATE(113)] = 3156,
  [SMALL_STATE(114)] = 3166,
  [SMALL_STATE(115)] = 3176,
  [SMALL_STATE(116)] = 3186,
  [SMALL_STATE(117)] = 3196,
  [SMALL_STATE(118)] = 3208,
  [SMALL_STATE(119)] = 3224,
  [SMALL_STATE(120)] = 3236,
  [SMALL_STATE(121)] = 3248,
  [SMALL_STATE(122)] = 3264,
  [SMALL_STATE(123)] = 3280,
  [SMALL_STATE(124)] = 3292,
  [SMALL_STATE(125)] = 3304,
  [SMALL_STATE(126)] = 3320,
  [SMALL_STATE(127)] = 3332,
  [SMALL_STATE(128)] = 3348,
  [SMALL_STATE(129)] = 3364,
  [SMALL_STATE(130)] = 3377,
  [SMALL_STATE(131)] = 3386,
  [SMALL_STATE(132)] = 3399,
  [SMALL_STATE(133)] = 3412,
  [SMALL_STATE(134)] = 3425,
  [SMALL_STATE(135)] = 3438,
  [SMALL_STATE(136)] = 3447,
  [SMALL_STATE(137)] = 3460,
  [SMALL_STATE(138)] = 3473,
  [SMALL_STATE(139)] = 3486,
  [SMALL_STATE(140)] = 3499,
  [SMALL_STATE(141)] = 3512,
  [SMALL_STATE(142)] = 3521,
  [SMALL_STATE(143)] = 3534,
  [SMALL_STATE(144)] = 3547,
  [SMALL_STATE(145)] = 3555,
  [SMALL_STATE(146)] = 3563,
  [SMALL_STATE(147)] = 3573,
  [SMALL_STATE(148)] = 3583,
  [SMALL_STATE(149)] = 3591,
  [SMALL_STATE(150)] = 3601,
  [SMALL_STATE(151)] = 3611,
  [SMALL_STATE(152)] = 3621,
  [SMALL_STATE(153)] = 3631,
  [SMALL_STATE(154)] = 3641,
  [SMALL_STATE(155)] = 3651,
  [SMALL_STATE(156)] = 3661,
  [SMALL_STATE(157)] = 3671,
  [SMALL_STATE(158)] = 3681,
  [SMALL_STATE(159)] = 3691,
  [SMALL_STATE(160)] = 3701,
  [SMALL_STATE(161)] = 3711,
  [SMALL_STATE(162)] = 3721,
  [SMALL_STATE(163)] = 3731,
  [SMALL_STATE(164)] = 3741,
  [SMALL_STATE(165)] = 3751,
  [SMALL_STATE(166)] = 3761,
  [SMALL_STATE(167)] = 3771,
  [SMALL_STATE(168)] = 3779,
  [SMALL_STATE(169)] = 3787,
  [SMALL_STATE(170)] = 3795,
  [SMALL_STATE(171)] = 3805,
  [SMALL_STATE(172)] = 3815,
  [SMALL_STATE(173)] = 3825,
  [SMALL_STATE(174)] = 3835,
  [SMALL_STATE(175)] = 3845,
  [SMALL_STATE(176)] = 3855,
  [SMALL_STATE(177)] = 3865,
  [SMALL_STATE(178)] = 3875,
  [SMALL_STATE(179)] = 3885,
  [SMALL_STATE(180)] = 3895,
  [SMALL_STATE(181)] = 3905,
  [SMALL_STATE(182)] = 3915,
  [SMALL_STATE(183)] = 3923,
  [SMALL_STATE(184)] = 3933,
  [SMALL_STATE(185)] = 3941,
  [SMALL_STATE(186)] = 3951,
  [SMALL_STATE(187)] = 3958,
  [SMALL_STATE(188)] = 3965,
  [SMALL_STATE(189)] = 3972,
  [SMALL_STATE(190)] = 3979,
  [SMALL_STATE(191)] = 3986,
  [SMALL_STATE(192)] = 3993,
  [SMALL_STATE(193)] = 4000,
  [SMALL_STATE(194)] = 4007,
  [SMALL_STATE(195)] = 4014,
  [SMALL_STATE(196)] = 4021,
  [SMALL_STATE(197)] = 4028,
  [SMALL_STATE(198)] = 4035,
  [SMALL_STATE(199)] = 4042,
  [SMALL_STATE(200)] = 4049,
  [SMALL_STATE(201)] = 4056,
  [SMALL_STATE(202)] = 4063,
  [SMALL_STATE(203)] = 4070,
  [SMALL_STATE(204)] = 4077,
  [SMALL_STATE(205)] = 4084,
  [SMALL_STATE(206)] = 4091,
  [SMALL_STATE(207)] = 4098,
  [SMALL_STATE(208)] = 4105,
  [SMALL_STATE(209)] = 4112,
  [SMALL_STATE(210)] = 4119,
  [SMALL_STATE(211)] = 4126,
  [SMALL_STATE(212)] = 4133,
  [SMALL_STATE(213)] = 4140,
  [SMALL_STATE(214)] = 4147,
  [SMALL_STATE(215)] = 4154,
  [SMALL_STATE(216)] = 4161,
  [SMALL_STATE(217)] = 4168,
  [SMALL_STATE(218)] = 4175,
  [SMALL_STATE(219)] = 4182,
  [SMALL_STATE(220)] = 4189,
  [SMALL_STATE(221)] = 4196,
  [SMALL_STATE(222)] = 4203,
  [SMALL_STATE(223)] = 4210,
  [SMALL_STATE(224)] = 4217,
  [SMALL_STATE(225)] = 4224,
  [SMALL_STATE(226)] = 4231,
  [SMALL_STATE(227)] = 4238,
  [SMALL_STATE(228)] = 4245,
  [SMALL_STATE(229)] = 4252,
  [SMALL_STATE(230)] = 4259,
  [SMALL_STATE(231)] = 4266,
  [SMALL_STATE(232)] = 4273,
  [SMALL_STATE(233)] = 4280,
  [SMALL_STATE(234)] = 4287,
  [SMALL_STATE(235)] = 4294,
  [SMALL_STATE(236)] = 4301,
  [SMALL_STATE(237)] = 4308,
  [SMALL_STATE(238)] = 4315,
  [SMALL_STATE(239)] = 4322,
  [SMALL_STATE(240)] = 4329,
  [SMALL_STATE(241)] = 4336,
  [SMALL_STATE(242)] = 4343,
  [SMALL_STATE(243)] = 4350,
  [SMALL_STATE(244)] = 4357,
  [SMALL_STATE(245)] = 4364,
  [SMALL_STATE(246)] = 4371,
  [SMALL_STATE(247)] = 4378,
  [SMALL_STATE(248)] = 4385,
  [SMALL_STATE(249)] = 4392,
  [SMALL_STATE(250)] = 4399,
  [SMALL_STATE(251)] = 4406,
};

static const TSParseActionEntry ts_parse_actions[] = {
  [0] = {.entry = {.count = 0, .reusable = false}},
  [1] = {.entry = {.count = 1, .reusable = false}}, RECOVER(),
  [3] = {.entry = {.count = 1, .reusable = true}}, SHIFT_EXTRA(),
  [5] = {.entry = {.count = 1, .reusable = true}}, SHIFT(194),
  [7] = {.entry = {.count = 1, .reusable = true}}, SHIFT(35),
  [9] = {.entry = {.count = 1, .reusable = false}}, SHIFT(110),
  [11] = {.entry = {.count = 1, .reusable = false}}, SHIFT(153),
  [13] = {.entry = {.count = 1, .reusable = true}}, SHIFT(61),
  [15] = {.entry = {.count = 1, .reusable = false}}, SHIFT(151),
  [17] = {.entry = {.count = 1, .reusable = false}}, SHIFT(39),
  [19] = {.entry = {.count = 1, .reusable = false}}, SHIFT(181),
  [21] = {.entry = {.count = 1, .reusable = false}}, SHIFT(188),
  [23] = {.entry = {.count = 1, .reusable = false}}, SHIFT(144),
  [25] = {.entry = {.count = 1, .reusable = false}}, SHIFT(93),
  [27] = {.entry = {.count = 1, .reusable = false}}, SHIFT(111),
  [29] = {.entry = {.count = 1, .reusable = true}}, SHIFT(140),
  [31] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_messageBody_repeat1, 2), SHIFT_REPEAT(35),
  [34] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_messageBody_repeat1, 2), SHIFT_REPEAT(110),
  [37] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_messageBody_repeat1, 2), SHIFT_REPEAT(153),
  [40] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_messageBody_repeat1, 2),
  [42] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_messageBody_repeat1, 2), SHIFT_REPEAT(151),
  [45] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_messageBody_repeat1, 2), SHIFT_REPEAT(39),
  [48] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_messageBody_repeat1, 2), SHIFT_REPEAT(181),
  [51] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_messageBody_repeat1, 2), SHIFT_REPEAT(188),
  [54] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_messageBody_repeat1, 2), SHIFT_REPEAT(144),
  [57] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_messageBody_repeat1, 2), SHIFT_REPEAT(93),
  [60] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_messageBody_repeat1, 2), SHIFT_REPEAT(111),
  [63] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_messageBody_repeat1, 2), SHIFT_REPEAT(140),
  [66] = {.entry = {.count = 1, .reusable = true}}, SHIFT(62),
  [68] = {.entry = {.count = 1, .reusable = true}}, SHIFT(31),
  [70] = {.entry = {.count = 1, .reusable = true}}, SHIFT(37),
  [72] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_fieldName, 1),
  [74] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_fieldName, 1),
  [76] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_range, 1),
  [78] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_range, 1),
  [80] = {.entry = {.count = 1, .reusable = false}}, SHIFT(169),
  [82] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_fieldNames, 1),
  [84] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_fieldNames, 1),
  [86] = {.entry = {.count = 1, .reusable = true}}, SHIFT(173),
  [88] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_ranges, 1),
  [90] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_ranges, 1),
  [92] = {.entry = {.count = 1, .reusable = true}}, SHIFT(171),
  [94] = {.entry = {.count = 1, .reusable = true}}, SHIFT(41),
  [96] = {.entry = {.count = 1, .reusable = false}}, SHIFT(121),
  [98] = {.entry = {.count = 1, .reusable = true}}, SHIFT(32),
  [100] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_ranges_repeat1, 2),
  [102] = {.entry = {.count = 1, .reusable = false}}, REDUCE(aux_sym_ranges_repeat1, 2),
  [104] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_ranges_repeat1, 2), SHIFT_REPEAT(171),
  [107] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_fieldNames, 2),
  [109] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_fieldNames, 2),
  [111] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_fieldNames_repeat1, 2),
  [113] = {.entry = {.count = 1, .reusable = false}}, REDUCE(aux_sym_fieldNames_repeat1, 2),
  [115] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_fieldNames_repeat1, 2), SHIFT_REPEAT(173),
  [118] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_oneof_repeat1, 2), SHIFT_REPEAT(41),
  [121] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_oneof_repeat1, 2), SHIFT_REPEAT(121),
  [124] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_oneof_repeat1, 2),
  [126] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_oneof_repeat1, 2), SHIFT_REPEAT(144),
  [129] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_oneof_repeat1, 2), SHIFT_REPEAT(111),
  [132] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_oneof_repeat1, 2), SHIFT_REPEAT(140),
  [135] = {.entry = {.count = 1, .reusable = true}}, SHIFT(27),
  [137] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_ranges, 2),
  [139] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_ranges, 2),
  [141] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_range, 3),
  [143] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_range, 3),
  [145] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_field, 9),
  [147] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_field, 9),
  [149] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_field, 5),
  [151] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_field, 5),
  [153] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_message, 3),
  [155] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_message, 3),
  [157] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_enum, 3),
  [159] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_enum, 3),
  [161] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_field, 8),
  [163] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_field, 8),
  [165] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_mapField, 13),
  [167] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_mapField, 13),
  [169] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_oneof, 4),
  [171] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_oneof, 4),
  [173] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_mapField, 10),
  [175] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_mapField, 10),
  [177] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_reserved, 2),
  [179] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_reserved, 2),
  [181] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_field, 6),
  [183] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_field, 6),
  [185] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_messageBody, 2),
  [187] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_messageBody, 2),
  [189] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_oneof, 5),
  [191] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_oneof, 5),
  [193] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_enumBody, 2),
  [195] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_enumBody, 2),
  [197] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_option, 5),
  [199] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_option, 5),
  [201] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_emptyStatement, 1),
  [203] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_emptyStatement, 1),
  [205] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_enumBody, 3),
  [207] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_enumBody, 3),
  [209] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_messageBody, 3),
  [211] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_messageBody, 3),
  [213] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_oneofField, 8),
  [215] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_oneofField, 8),
  [217] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_oneofField, 5),
  [219] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_oneofField, 5),
  [221] = {.entry = {.count = 1, .reusable = true}}, SHIFT(118),
  [223] = {.entry = {.count = 1, .reusable = false}}, SHIFT(97),
  [225] = {.entry = {.count = 1, .reusable = true}}, SHIFT(104),
  [227] = {.entry = {.count = 1, .reusable = false}}, SHIFT(104),
  [229] = {.entry = {.count = 1, .reusable = true}}, SHIFT(228),
  [231] = {.entry = {.count = 1, .reusable = true}}, SHIFT(227),
  [233] = {.entry = {.count = 1, .reusable = true}}, SHIFT(80),
  [235] = {.entry = {.count = 1, .reusable = false}}, SHIFT(59),
  [237] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_source_file, 2),
  [239] = {.entry = {.count = 1, .reusable = true}}, SHIFT(55),
  [241] = {.entry = {.count = 1, .reusable = true}}, SHIFT(150),
  [243] = {.entry = {.count = 1, .reusable = true}}, SHIFT(129),
  [245] = {.entry = {.count = 1, .reusable = true}}, SHIFT(127),
  [247] = {.entry = {.count = 1, .reusable = true}}, SHIFT(155),
  [249] = {.entry = {.count = 1, .reusable = true}}, SHIFT(156),
  [251] = {.entry = {.count = 1, .reusable = true}}, SHIFT(162),
  [253] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_source_file, 1),
  [255] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2),
  [257] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2), SHIFT_REPEAT(55),
  [260] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2), SHIFT_REPEAT(150),
  [263] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2), SHIFT_REPEAT(129),
  [266] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2), SHIFT_REPEAT(127),
  [269] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2), SHIFT_REPEAT(155),
  [272] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2), SHIFT_REPEAT(156),
  [275] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2), SHIFT_REPEAT(162),
  [278] = {.entry = {.count = 1, .reusable = true}}, SHIFT(187),
  [280] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_fullIdent_repeat1, 2),
  [282] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_fullIdent_repeat1, 2), SHIFT_REPEAT(210),
  [285] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_fullIdent, 2),
  [287] = {.entry = {.count = 1, .reusable = true}}, SHIFT(210),
  [289] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_fullIdent, 1),
  [291] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_service_repeat1, 2), SHIFT_REPEAT(55),
  [294] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_service_repeat1, 2), SHIFT_REPEAT(127),
  [297] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_service_repeat1, 2),
  [299] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_service_repeat1, 2), SHIFT_REPEAT(154),
  [302] = {.entry = {.count = 1, .reusable = true}}, SHIFT(109),
  [304] = {.entry = {.count = 1, .reusable = false}}, SHIFT(125),
  [306] = {.entry = {.count = 1, .reusable = true}}, SHIFT(36),
  [308] = {.entry = {.count = 1, .reusable = false}}, SHIFT(192),
  [310] = {.entry = {.count = 1, .reusable = true}}, SHIFT(68),
  [312] = {.entry = {.count = 1, .reusable = true}}, SHIFT(154),
  [314] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_service, 4),
  [316] = {.entry = {.count = 1, .reusable = true}}, SHIFT(73),
  [318] = {.entry = {.count = 1, .reusable = true}}, SHIFT(76),
  [320] = {.entry = {.count = 1, .reusable = true}}, SHIFT(33),
  [322] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_import, 3),
  [324] = {.entry = {.count = 1, .reusable = true}}, SHIFT(78),
  [326] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_package, 3),
  [328] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_service, 5),
  [330] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_syntax, 4),
  [332] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_enumBody_repeat1, 2), SHIFT_REPEAT(109),
  [335] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_enumBody_repeat1, 2), SHIFT_REPEAT(125),
  [338] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_enumBody_repeat1, 2),
  [340] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_enumBody_repeat1, 2), SHIFT_REPEAT(192),
  [343] = {.entry = {.count = 1, .reusable = false}}, SHIFT(102),
  [345] = {.entry = {.count = 1, .reusable = true}}, SHIFT(114),
  [347] = {.entry = {.count = 1, .reusable = true}}, SHIFT(164),
  [349] = {.entry = {.count = 1, .reusable = true}}, SHIFT(59),
  [351] = {.entry = {.count = 1, .reusable = false}}, SHIFT(96),
  [353] = {.entry = {.count = 1, .reusable = false}}, SHIFT(107),
  [355] = {.entry = {.count = 1, .reusable = true}}, SHIFT(115),
  [357] = {.entry = {.count = 1, .reusable = true}}, SHIFT(112),
  [359] = {.entry = {.count = 1, .reusable = true}}, SHIFT(113),
  [361] = {.entry = {.count = 1, .reusable = true}}, SHIFT(8),
  [363] = {.entry = {.count = 1, .reusable = true}}, SHIFT(7),
  [365] = {.entry = {.count = 1, .reusable = false}}, SHIFT(101),
  [367] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_rpc_repeat1, 2), SHIFT_REPEAT(55),
  [370] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_rpc_repeat1, 2), SHIFT_REPEAT(127),
  [373] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_rpc_repeat1, 2),
  [375] = {.entry = {.count = 1, .reusable = true}}, SHIFT(111),
  [377] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_constant, 1),
  [379] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_msgLit, 2),
  [381] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_constant, 2),
  [383] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_floatLit, 1),
  [385] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_msgLit, 3),
  [387] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_strLit, 3),
  [389] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_messageName, 1),
  [391] = {.entry = {.count = 1, .reusable = true}}, SHIFT(202),
  [393] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_rpc, 14),
  [395] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_rpc, 13),
  [397] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_rpc, 12),
  [399] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_rpc, 11),
  [401] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_rpc, 10),
  [403] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_enumField, 5),
  [405] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_enumField, 5),
  [407] = {.entry = {.count = 1, .reusable = true}}, SHIFT(98),
  [409] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_enumField, 9),
  [411] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_enumField, 9),
  [413] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_enumField, 4),
  [415] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_enumField, 4),
  [417] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_msgLit_repeat1, 2),
  [419] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_msgLit_repeat1, 2), SHIFT_REPEAT(7),
  [422] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_enumField, 8),
  [424] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_enumField, 8),
  [426] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_enumField, 7),
  [428] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_enumField, 7),
  [430] = {.entry = {.count = 1, .reusable = true}}, SHIFT(105),
  [432] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_enumMessageType, 1),
  [434] = {.entry = {.count = 1, .reusable = true}}, SHIFT(100),
  [436] = {.entry = {.count = 1, .reusable = true}}, SHIFT(218),
  [438] = {.entry = {.count = 1, .reusable = true}}, SHIFT(190),
  [440] = {.entry = {.count = 1, .reusable = true}}, SHIFT(103),
  [442] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_fieldOptions, 2),
  [444] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_fieldOptions_repeat1, 2), SHIFT_REPEAT(103),
  [447] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_fieldOptions_repeat1, 2),
  [449] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_enumMessageType, 3),
  [451] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_fieldOptions, 1),
  [453] = {.entry = {.count = 1, .reusable = true}}, SHIFT(233),
  [455] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_enumMessageType, 2),
  [457] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_enumField_repeat1, 2), SHIFT_REPEAT(100),
  [460] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_enumField_repeat1, 2),
  [462] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_type, 1),
  [464] = {.entry = {.count = 1, .reusable = true}}, SHIFT(237),
  [466] = {.entry = {.count = 1, .reusable = true}}, SHIFT(163),
  [468] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_enumMessageType_repeat1, 2), SHIFT_REPEAT(197),
  [471] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_fieldNumber, 1),
  [473] = {.entry = {.count = 1, .reusable = true}}, SHIFT(22),
  [475] = {.entry = {.count = 1, .reusable = true}}, SHIFT(88),
  [477] = {.entry = {.count = 1, .reusable = true}}, SHIFT(216),
  [479] = {.entry = {.count = 1, .reusable = true}}, SHIFT(30),
  [481] = {.entry = {.count = 1, .reusable = true}}, SHIFT(82),
  [483] = {.entry = {.count = 1, .reusable = true}}, SHIFT(221),
  [485] = {.entry = {.count = 1, .reusable = true}}, SHIFT(249),
  [487] = {.entry = {.count = 1, .reusable = true}}, SHIFT(5),
  [489] = {.entry = {.count = 1, .reusable = true}}, SHIFT(71),
  [491] = {.entry = {.count = 1, .reusable = true}}, SHIFT(148),
  [493] = {.entry = {.count = 1, .reusable = true}}, SHIFT(212),
  [495] = {.entry = {.count = 1, .reusable = true}}, SHIFT(214),
  [497] = {.entry = {.count = 1, .reusable = true}}, SHIFT(120),
  [499] = {.entry = {.count = 1, .reusable = true}}, SHIFT(106),
  [501] = {.entry = {.count = 1, .reusable = true}}, SHIFT(69),
  [503] = {.entry = {.count = 1, .reusable = true}}, SHIFT(2),
  [505] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_enumValueOption, 3),
  [507] = {.entry = {.count = 1, .reusable = true}}, SHIFT(18),
  [509] = {.entry = {.count = 1, .reusable = true}}, SHIFT(42),
  [511] = {.entry = {.count = 1, .reusable = true}}, SHIFT(91),
  [513] = {.entry = {.count = 1, .reusable = true}}, SHIFT(90),
  [515] = {.entry = {.count = 1, .reusable = true}}, SHIFT(81),
  [517] = {.entry = {.count = 1, .reusable = true}}, SHIFT(28),
  [519] = {.entry = {.count = 1, .reusable = true}}, SHIFT(87),
  [521] = {.entry = {.count = 1, .reusable = true}}, SHIFT(116),
  [523] = {.entry = {.count = 1, .reusable = true}}, SHIFT(85),
  [525] = {.entry = {.count = 1, .reusable = true}}, SHIFT(117),
  [527] = {.entry = {.count = 1, .reusable = true}}, SHIFT(99),
  [529] = {.entry = {.count = 1, .reusable = true}}, SHIFT(189),
  [531] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_msgLit_repeat1, 3),
  [533] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_fieldOption, 3),
  [535] = {.entry = {.count = 1, .reusable = true}}, SHIFT(174),
  [537] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_keyType, 1),
  [539] = {.entry = {.count = 1, .reusable = true}}, SHIFT(54),
  [541] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_oneofName, 1),
  [543] = {.entry = {.count = 1, .reusable = true}}, SHIFT(119),
  [545] = {.entry = {.count = 1, .reusable = true}}, SHIFT(21),
  [547] = {.entry = {.count = 1, .reusable = true}}, SHIFT(146),
  [549] = {.entry = {.count = 1, .reusable = true}}, SHIFT(56),
  [551] = {.entry = {.count = 1, .reusable = true}}, SHIFT(244),
  [553] = {.entry = {.count = 1, .reusable = true}}, SHIFT(179),
  [555] = {.entry = {.count = 1, .reusable = true}}, SHIFT(198),
  [557] = {.entry = {.count = 1, .reusable = true}}, SHIFT(40),
  [559] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_optionName, 3),
  [561] = {.entry = {.count = 1, .reusable = true}}, SHIFT(47),
  [563] = {.entry = {.count = 1, .reusable = true}}, SHIFT(172),
  [565] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_enumMessageType_repeat1, 2),
  [567] = {.entry = {.count = 1, .reusable = true}}, SHIFT(25),
  [569] = {.entry = {.count = 1, .reusable = true}}, SHIFT(199),
  [571] = {.entry = {.count = 1, .reusable = true}}, SHIFT(108),
  [573] = {.entry = {.count = 1, .reusable = true}}, SHIFT(211),
  [575] = {.entry = {.count = 1, .reusable = true}}, SHIFT(16),
  [577] = {.entry = {.count = 1, .reusable = true}}, SHIFT(183),
  [579] = {.entry = {.count = 1, .reusable = true}}, SHIFT(60),
  [581] = {.entry = {.count = 1, .reusable = true}}, SHIFT(26),
  [583] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_mapName, 1),
  [585] = {.entry = {.count = 1, .reusable = true}}, SHIFT(65),
  [587] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_serviceName, 1),
  [589] = {.entry = {.count = 1, .reusable = true}}, SHIFT(191),
  [591] = {.entry = {.count = 1, .reusable = true}}, SHIFT(38),
  [593] = {.entry = {.count = 1, .reusable = true}}, SHIFT(124),
  [595] = {.entry = {.count = 1, .reusable = true}}, SHIFT(84),
  [597] = {.entry = {.count = 1, .reusable = true}}, SHIFT(178),
  [599] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_enumName, 1),
  [601] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_optionName, 1),
  [603] = {.entry = {.count = 1, .reusable = true}}, SHIFT(44),
  [605] = {.entry = {.count = 1, .reusable = true}}, SHIFT(83),
  [607] = {.entry = {.count = 1, .reusable = true}}, SHIFT(219),
  [609] = {.entry = {.count = 1, .reusable = true}}, SHIFT(72),
  [611] = {.entry = {.count = 1, .reusable = false}}, SHIFT_EXTRA(),
  [613] = {.entry = {.count = 1, .reusable = false}}, SHIFT(205),
  [615] = {.entry = {.count = 1, .reusable = false}}, SHIFT(209),
  [617] = {.entry = {.count = 1, .reusable = true}}, SHIFT(75),
  [619] = {.entry = {.count = 1, .reusable = true}}, SHIFT(77),
  [621] = {.entry = {.count = 1, .reusable = true}}, SHIFT(203),
  [623] = {.entry = {.count = 1, .reusable = true}}, SHIFT(49),
  [625] = {.entry = {.count = 1, .reusable = true}}, SHIFT(126),
  [627] = {.entry = {.count = 1, .reusable = true}}, SHIFT(224),
  [629] = {.entry = {.count = 1, .reusable = true}}, SHIFT(225),
  [631] = {.entry = {.count = 1, .reusable = true}}, SHIFT(123),
  [633] = {.entry = {.count = 1, .reusable = true}}, SHIFT(180),
  [635] = {.entry = {.count = 1, .reusable = true}}, SHIFT(94),
  [637] = {.entry = {.count = 1, .reusable = true}}, SHIFT(34),
  [639] = {.entry = {.count = 1, .reusable = true}}, SHIFT(43),
  [641] = {.entry = {.count = 1, .reusable = true}}, SHIFT(161),
  [643] = {.entry = {.count = 1, .reusable = true}}, SHIFT(160),
  [645] = {.entry = {.count = 1, .reusable = true}}, SHIFT(46),
  [647] = {.entry = {.count = 1, .reusable = true}}, SHIFT(230),
  [649] = {.entry = {.count = 1, .reusable = true}},  ACCEPT_INPUT(),
  [651] = {.entry = {.count = 1, .reusable = true}}, SHIFT(48),
  [653] = {.entry = {.count = 1, .reusable = true}}, SHIFT(50),
  [655] = {.entry = {.count = 1, .reusable = true}}, SHIFT(45),
  [657] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_rpcName, 1),
  [659] = {.entry = {.count = 1, .reusable = true}}, SHIFT(234),
  [661] = {.entry = {.count = 1, .reusable = true}}, SHIFT(175),
};

#ifdef __cplusplus
extern "C" {
#endif
#ifdef _WIN32
#define extern __declspec(dllexport)
#endif

extern const TSLanguage *tree_sitter_protobuf(void) {
  static const TSLanguage language = {
    .version = LANGUAGE_VERSION,
    .symbol_count = SYMBOL_COUNT,
    .alias_count = ALIAS_COUNT,
    .token_count = TOKEN_COUNT,
    .external_token_count = EXTERNAL_TOKEN_COUNT,
    .state_count = STATE_COUNT,
    .large_state_count = LARGE_STATE_COUNT,
    .production_id_count = PRODUCTION_ID_COUNT,
    .field_count = FIELD_COUNT,
    .max_alias_sequence_length = MAX_ALIAS_SEQUENCE_LENGTH,
    .parse_table = &ts_parse_table[0][0],
    .small_parse_table = ts_small_parse_table,
    .small_parse_table_map = ts_small_parse_table_map,
    .parse_actions = ts_parse_actions,
    .symbol_names = ts_symbol_names,
    .symbol_metadata = ts_symbol_metadata,
    .public_symbol_map = ts_symbol_map,
    .alias_map = ts_non_terminal_alias_map,
    .alias_sequences = &ts_alias_sequences[0][0],
    .lex_modes = ts_lex_modes,
    .lex_fn = ts_lex,
  };
  return &language;
}
#ifdef __cplusplus
}
#endif
