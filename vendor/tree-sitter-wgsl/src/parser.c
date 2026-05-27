#include <tree_sitter/parser.h>

#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wmissing-field-initializers"
#endif

#define LANGUAGE_VERSION 13
#define STATE_COUNT 422
#define LARGE_STATE_COUNT 47
#define SYMBOL_COUNT 593
#define ALIAS_COUNT 0
#define TOKEN_COUNT 495
#define EXTERNAL_TOKEN_COUNT 1
#define FIELD_COUNT 0
#define MAX_ALIAS_SEQUENCE_LENGTH 8
#define PRODUCTION_ID_COUNT 1

enum {
  sym_ident = 1,
  aux_sym_int_literal_token1 = 2,
  aux_sym_int_literal_token2 = 3,
  aux_sym_int_literal_token3 = 4,
  aux_sym_decimal_float_literal_token1 = 5,
  aux_sym_decimal_float_literal_token2 = 6,
  aux_sym_decimal_float_literal_token3 = 7,
  aux_sym_decimal_float_literal_token4 = 8,
  aux_sym_decimal_float_literal_token5 = 9,
  aux_sym_hex_float_literal_token1 = 10,
  aux_sym_hex_float_literal_token2 = 11,
  aux_sym_hex_float_literal_token3 = 12,
  anon_sym_read = 13,
  anon_sym_write = 14,
  anon_sym_read_write = 15,
  anon_sym_rgba8unorm = 16,
  anon_sym_rgba8snorm = 17,
  anon_sym_rgba8uint = 18,
  anon_sym_rgba8sint = 19,
  anon_sym_rgba16uint = 20,
  anon_sym_rgba16sint = 21,
  anon_sym_rgba16float = 22,
  anon_sym_r32uint = 23,
  anon_sym_r32sint = 24,
  anon_sym_r32float = 25,
  anon_sym_rg32uint = 26,
  anon_sym_rg32sint = 27,
  anon_sym_rg32float = 28,
  anon_sym_rgba32uint = 29,
  anon_sym_rgba32sint = 30,
  anon_sym_rgba32float = 31,
  sym_array = 32,
  sym_atomic = 33,
  sym_bool = 34,
  sym_float32 = 35,
  sym_float16 = 36,
  sym_int32 = 37,
  sym_mat2x2 = 38,
  sym_mat2x3 = 39,
  sym_mat2x4 = 40,
  sym_mat3x2 = 41,
  sym_mat3x3 = 42,
  sym_mat3x4 = 43,
  sym_mat4x2 = 44,
  sym_mat4x3 = 45,
  sym_mat4x4 = 46,
  sym_override = 47,
  sym_pointer = 48,
  sym_sampler = 49,
  sym_sampler_comparison = 50,
  sym_staticAssert = 51,
  sym_struct = 52,
  sym_texture_1d = 53,
  sym_texture_2d = 54,
  sym_texture_2d_array = 55,
  sym_texture_3d = 56,
  sym_texture_cube = 57,
  sym_texture_cube_array = 58,
  sym_texture_multisampled_2d = 59,
  sym_texture_storage_1d = 60,
  sym_texture_storage_2d = 61,
  sym_texture_storage_2d_array = 62,
  sym_texture_storage_3d = 63,
  sym_texture_depth_2d = 64,
  sym_texture_depth_2d_array = 65,
  sym_texture_depth_cube = 66,
  sym_texture_depth_cube_array = 67,
  sym_texture_depth_multisampled_2d = 68,
  sym_uint32 = 69,
  sym_vec2 = 70,
  sym_vec3 = 71,
  sym_vec4 = 72,
  sym_bitcast = 73,
  sym_break = 74,
  sym_case = 75,
  sym_const = 76,
  sym_continue = 77,
  sym_continuing = 78,
  sym_default = 79,
  sym_discard = 80,
  sym_else = 81,
  sym_enable = 82,
  sym_fallthrough = 83,
  sym_false = 84,
  sym_fn = 85,
  sym_for = 86,
  sym_function = 87,
  sym_if = 88,
  sym_let = 89,
  sym_loop = 90,
  sym_private = 91,
  sym_return = 92,
  sym_storage = 93,
  sym_switch = 94,
  sym_true = 95,
  sym_type = 96,
  sym_uniform = 97,
  sym_var = 98,
  sym_while = 99,
  sym_workgroup = 100,
  sym_and = 101,
  sym_and_and = 102,
  sym_arrow = 103,
  sym_attr = 104,
  sym_forward_slash = 105,
  sym_bang = 106,
  sym_bracket_left = 107,
  sym_bracket_right = 108,
  sym_brace_left = 109,
  sym_brace_right = 110,
  sym_colon = 111,
  sym_comma = 112,
  sym_equal = 113,
  sym_equal_equal = 114,
  sym_not_equal = 115,
  sym_greater_than = 116,
  sym_greater_than_equal = 117,
  sym_less_than = 118,
  sym_less_than_equal = 119,
  sym_modulo = 120,
  sym_minus = 121,
  sym_minus_minus = 122,
  sym_period = 123,
  sym_plus = 124,
  sym_plus_plus = 125,
  sym_or = 126,
  sym_or_or = 127,
  sym_paren_left = 128,
  sym_paren_right = 129,
  sym_semicolon = 130,
  sym_star = 131,
  sym_tilde = 132,
  sym_underscore = 133,
  sym_xor = 134,
  sym_plus_equal = 135,
  sym_minus_equal = 136,
  sym_times_equal = 137,
  sym_division_equal = 138,
  sym_modulo_equal = 139,
  sym_and_equal = 140,
  sym_or_equal = 141,
  sym_xor_equal = 142,
  anon_sym_AppendStructuredBuffer = 143,
  anon_sym_BlendState = 144,
  anon_sym_Buffer = 145,
  anon_sym_ByteAddressBuffer = 146,
  anon_sym_CompileShader = 147,
  anon_sym_ComputeShader = 148,
  anon_sym_ConsumeStructuredBuffer = 149,
  anon_sym_DepthStencilState = 150,
  anon_sym_DepthStencilView = 151,
  anon_sym_DomainShader = 152,
  anon_sym_GeometryShader = 153,
  anon_sym_Hullshader = 154,
  anon_sym_InputPatch = 155,
  anon_sym_LineStream = 156,
  anon_sym_NULL = 157,
  anon_sym_OutputPatch = 158,
  anon_sym_PixelShader = 159,
  anon_sym_PointStream = 160,
  anon_sym_RWBuffer = 161,
  anon_sym_RWByteAddressBuffer = 162,
  anon_sym_RWStructuredBuffer = 163,
  anon_sym_RWTexture1D = 164,
  anon_sym_RWTexture1DArray = 165,
  anon_sym_RWTexture2D = 166,
  anon_sym_RWTexture2DArray = 167,
  anon_sym_RWTexture3D = 168,
  anon_sym_RasterizerState = 169,
  anon_sym_RenderTargetView = 170,
  anon_sym_SamplerComparisonState = 171,
  anon_sym_SamplerState = 172,
  anon_sym_Self = 173,
  anon_sym_StructuredBuffer = 174,
  anon_sym_Texture1D = 175,
  anon_sym_Texture1DArray = 176,
  anon_sym_Texture2D = 177,
  anon_sym_Texture2DArray = 178,
  anon_sym_Texture2DMS = 179,
  anon_sym_Texture2DMSArray = 180,
  anon_sym_Texture3D = 181,
  anon_sym_TextureCube = 182,
  anon_sym_TextureCubeArray = 183,
  anon_sym_TriangleStream = 184,
  anon_sym_VertexShader = 185,
  anon_sym_abstract = 186,
  anon_sym_active = 187,
  anon_sym_alignas = 188,
  anon_sym_alignof = 189,
  anon_sym_as = 190,
  anon_sym_asm = 191,
  anon_sym_asm_fragment = 192,
  anon_sym_async = 193,
  anon_sym_atomic_uint = 194,
  anon_sym_attribute = 195,
  anon_sym_auto = 196,
  anon_sym_await = 197,
  anon_sym_become = 198,
  anon_sym_bf16 = 199,
  anon_sym_binding_array = 200,
  anon_sym_cast = 201,
  anon_sym_catch = 202,
  anon_sym_cbuffer = 203,
  anon_sym_centroid = 204,
  anon_sym_char = 205,
  anon_sym_class = 206,
  anon_sym_co_await = 207,
  anon_sym_co_return = 208,
  anon_sym_co_yield = 209,
  anon_sym_coherent = 210,
  anon_sym_column_major = 211,
  anon_sym_common = 212,
  anon_sym_compile = 213,
  anon_sym_compile_fragment = 214,
  anon_sym_concept = 215,
  anon_sym_const_cast = 216,
  anon_sym_consteval = 217,
  anon_sym_constexpr = 218,
  anon_sym_constinit = 219,
  anon_sym_crate = 220,
  anon_sym_debugger = 221,
  anon_sym_decltype = 222,
  anon_sym_delete = 223,
  anon_sym_demote = 224,
  anon_sym_demote_to_helper = 225,
  anon_sym_do = 226,
  anon_sym_dword = 227,
  anon_sym_dynamic_cast = 228,
  anon_sym_enum = 229,
  anon_sym_explicit = 230,
  anon_sym_export = 231,
  anon_sym_extends = 232,
  anon_sym_extern = 233,
  anon_sym_external = 234,
  anon_sym_f64 = 235,
  anon_sym_filter = 236,
  anon_sym_final = 237,
  anon_sym_finally = 238,
  anon_sym_fixed = 239,
  anon_sym_flat = 240,
  anon_sym_friend = 241,
  anon_sym_from = 242,
  anon_sym_fvec2 = 243,
  anon_sym_fvec3 = 244,
  anon_sym_fvec4 = 245,
  anon_sym_fxgroup = 246,
  anon_sym_get = 247,
  anon_sym_goto = 248,
  anon_sym_groupshared = 249,
  anon_sym_handle = 250,
  anon_sym_highp = 251,
  anon_sym_hvec2 = 252,
  anon_sym_hvec3 = 253,
  anon_sym_hvec4 = 254,
  anon_sym_i16 = 255,
  anon_sym_i64 = 256,
  anon_sym_i8 = 257,
  anon_sym_iimage1D = 258,
  anon_sym_iimage1DArray = 259,
  anon_sym_iimage2D = 260,
  anon_sym_iimage2DArray = 261,
  anon_sym_iimage2DMS = 262,
  anon_sym_iimage2DMSArray = 263,
  anon_sym_iimage2DRect = 264,
  anon_sym_iimage3D = 265,
  anon_sym_iimageBuffer = 266,
  anon_sym_iimageCube = 267,
  anon_sym_iimageCubeArray = 268,
  anon_sym_image1D = 269,
  anon_sym_image1DArray = 270,
  anon_sym_image2D = 271,
  anon_sym_image2DArray = 272,
  anon_sym_image2DMS = 273,
  anon_sym_image2DMSArray = 274,
  anon_sym_image2DRect = 275,
  anon_sym_image3D = 276,
  anon_sym_imageBuffer = 277,
  anon_sym_imageCube = 278,
  anon_sym_imageCubeArray = 279,
  anon_sym_impl = 280,
  anon_sym_implements = 281,
  anon_sym_import = 282,
  anon_sym_inline = 283,
  anon_sym_inout = 284,
  anon_sym_instanceof = 285,
  anon_sym_interface = 286,
  anon_sym_invariant = 287,
  anon_sym_isampler1D = 288,
  anon_sym_isampler1DArray = 289,
  anon_sym_isampler2D = 290,
  anon_sym_isampler2DArray = 291,
  anon_sym_isampler2DMS = 292,
  anon_sym_isampler2DMSArray = 293,
  anon_sym_isampler2DRect = 294,
  anon_sym_isampler3D = 295,
  anon_sym_isamplerBuffer = 296,
  anon_sym_isamplerCube = 297,
  anon_sym_isamplerCubeArray = 298,
  anon_sym_isubpassInput = 299,
  anon_sym_isubpassInputMS = 300,
  anon_sym_itexture1D = 301,
  anon_sym_itexture1DArray = 302,
  anon_sym_itexture2D = 303,
  anon_sym_itexture2DArray = 304,
  anon_sym_itexture2DMS = 305,
  anon_sym_itexture2DMSArray = 306,
  anon_sym_itexture2DRect = 307,
  anon_sym_itexture3D = 308,
  anon_sym_itextureBuffer = 309,
  anon_sym_itextureCube = 310,
  anon_sym_itextureCubeArray = 311,
  anon_sym_layout = 312,
  anon_sym_line = 313,
  anon_sym_lineadj = 314,
  anon_sym_linear = 315,
  anon_sym_lowp = 316,
  anon_sym_macro = 317,
  anon_sym_macro_rules = 318,
  anon_sym_mat = 319,
  anon_sym_match = 320,
  anon_sym_matrix = 321,
  anon_sym_mediump = 322,
  anon_sym_meta = 323,
  anon_sym_mod = 324,
  anon_sym_module = 325,
  anon_sym_move = 326,
  anon_sym_mut = 327,
  anon_sym_mutable = 328,
  anon_sym_namespace = 329,
  anon_sym_new = 330,
  anon_sym_nil = 331,
  anon_sym_noexcept = 332,
  anon_sym_noinline = 333,
  anon_sym_nointerpolation = 334,
  anon_sym_noperspective = 335,
  anon_sym_null = 336,
  anon_sym_nullptr = 337,
  anon_sym_of = 338,
  anon_sym_operator = 339,
  anon_sym_package = 340,
  anon_sym_packoffset = 341,
  anon_sym_partition = 342,
  anon_sym_pass = 343,
  anon_sym_patch = 344,
  anon_sym_pixelfragment = 345,
  anon_sym_point = 346,
  anon_sym_precise = 347,
  anon_sym_precision = 348,
  anon_sym_premerge = 349,
  anon_sym_priv = 350,
  anon_sym_protected = 351,
  anon_sym_pub = 352,
  anon_sym_public = 353,
  anon_sym_readonly = 354,
  anon_sym_ref = 355,
  anon_sym_regardless = 356,
  anon_sym_register = 357,
  anon_sym_reinterpret_cast = 358,
  anon_sym_requires = 359,
  anon_sym_resource = 360,
  anon_sym_restrict = 361,
  anon_sym_row_major = 362,
  anon_sym_samper = 363,
  anon_sym_sample = 364,
  anon_sym_sampler1D = 365,
  anon_sym_sampler1DArray = 366,
  anon_sym_sampler1DArrayShadow = 367,
  anon_sym_sampler1DShadow = 368,
  anon_sym_sampler2D = 369,
  anon_sym_sampler2DArray = 370,
  anon_sym_sampler2DArrayShadow = 371,
  anon_sym_sampler2DMS = 372,
  anon_sym_sampler2DMSArray = 373,
  anon_sym_sampler2DRect = 374,
  anon_sym_sampler2DRectShadow = 375,
  anon_sym_sampler2DShadow = 376,
  anon_sym_sampler3D = 377,
  anon_sym_sampler3DRect = 378,
  anon_sym_samplerBuffer = 379,
  anon_sym_samplerCube = 380,
  anon_sym_samplerCubeArray = 381,
  anon_sym_samplerCubeArrayShadow = 382,
  anon_sym_samplerCubeShadow = 383,
  anon_sym_samplerShadow = 384,
  anon_sym_self = 385,
  anon_sym_set = 386,
  anon_sym_shared = 387,
  anon_sym_signed = 388,
  anon_sym_sizeof = 389,
  anon_sym_smooth = 390,
  anon_sym_snorm = 391,
  anon_sym_stateblock = 392,
  anon_sym_stateblock_state = 393,
  anon_sym_static = 394,
  anon_sym_static_assert = 395,
  anon_sym_static_cast = 396,
  anon_sym_std = 397,
  anon_sym_string = 398,
  anon_sym_subpassInput = 399,
  anon_sym_subpassInputMS = 400,
  anon_sym_subroutine = 401,
  anon_sym_super = 402,
  anon_sym_superp = 403,
  anon_sym_target = 404,
  anon_sym_tbuffer = 405,
  anon_sym_technique = 406,
  anon_sym_technique10 = 407,
  anon_sym_technique11 = 408,
  anon_sym_template = 409,
  anon_sym_texture1D = 410,
  anon_sym_texture1DArray = 411,
  anon_sym_texture2D = 412,
  anon_sym_texture2DArray = 413,
  anon_sym_texture2DMS = 414,
  anon_sym_texture2DMSArray = 415,
  anon_sym_texture2DRect = 416,
  anon_sym_texture3D = 417,
  anon_sym_textureBuffer = 418,
  anon_sym_textureCube = 419,
  anon_sym_textureCubeArray = 420,
  anon_sym_this = 421,
  anon_sym_thread_local = 422,
  anon_sym_throw = 423,
  anon_sym_trait = 424,
  anon_sym_triangle = 425,
  anon_sym_triangleadj = 426,
  anon_sym_try = 427,
  anon_sym_typedef = 428,
  anon_sym_typeid = 429,
  anon_sym_typename = 430,
  anon_sym_typeof = 431,
  anon_sym_u16 = 432,
  anon_sym_u64 = 433,
  anon_sym_u8 = 434,
  anon_sym_uimage1D = 435,
  anon_sym_uimage1DArray = 436,
  anon_sym_uimage2D = 437,
  anon_sym_uimage2DArray = 438,
  anon_sym_uimage2DMS = 439,
  anon_sym_uimage2DMSArray = 440,
  anon_sym_uimage2DRect = 441,
  anon_sym_uimage3D = 442,
  anon_sym_uimageBuffer = 443,
  anon_sym_uimageCube = 444,
  anon_sym_uimageCubeArray = 445,
  anon_sym_union = 446,
  anon_sym_unless = 447,
  anon_sym_unorm = 448,
  anon_sym_unsafe = 449,
  anon_sym_unsigned = 450,
  anon_sym_unsized = 451,
  anon_sym_usampler1D = 452,
  anon_sym_usampler1DArray = 453,
  anon_sym_usampler2D = 454,
  anon_sym_usampler2DArray = 455,
  anon_sym_usampler2DMS = 456,
  anon_sym_usampler2DMSArray = 457,
  anon_sym_usampler2DRect = 458,
  anon_sym_usampler3D = 459,
  anon_sym_usamplerBuffer = 460,
  anon_sym_usamplerCube = 461,
  anon_sym_usamplerCubeArray = 462,
  anon_sym_use = 463,
  anon_sym_using = 464,
  anon_sym_usubpassInput = 465,
  anon_sym_usubpassInputMS = 466,
  anon_sym_utexture1D = 467,
  anon_sym_utexture1DArray = 468,
  anon_sym_utexture2D = 469,
  anon_sym_utexture2DArray = 470,
  anon_sym_utexture2DMS = 471,
  anon_sym_utexture2DMSArray = 472,
  anon_sym_utexture2DRect = 473,
  anon_sym_utexture3D = 474,
  anon_sym_utextureBuffer = 475,
  anon_sym_utextureCube = 476,
  anon_sym_utextureCubeArray = 477,
  anon_sym_varying = 478,
  anon_sym_vec = 479,
  anon_sym_vector = 480,
  anon_sym_vertexfragment = 481,
  anon_sym_virtual = 482,
  anon_sym_void = 483,
  anon_sym_volatile = 484,
  anon_sym_wchar_t = 485,
  anon_sym_wgsl = 486,
  anon_sym_where = 487,
  anon_sym_with = 488,
  anon_sym_writeonly = 489,
  anon_sym_yield = 490,
  anon_sym_SLASH_SLASH = 491,
  aux_sym__comment_token1 = 492,
  sym__blankspace = 493,
  sym__block_comment = 494,
  sym_translation_unit = 495,
  sym_global_directive = 496,
  sym_bool_literal = 497,
  sym_int_literal = 498,
  sym_float_literal = 499,
  sym_decimal_float_literal = 500,
  sym_hex_float_literal = 501,
  sym_const_literal = 502,
  sym_attribute = 503,
  sym_literal_or_ident = 504,
  sym_array_type_decl = 505,
  sym_element_count_expression = 506,
  sym_struct_decl = 507,
  sym_struct_body_decl = 508,
  sym_struct_member = 509,
  sym_address_space = 510,
  sym_texture_sampler_types = 511,
  sym_sampler_type = 512,
  sym_sampled_texture_type = 513,
  sym_multisampled_texture_type = 514,
  sym_storage_texture_type = 515,
  sym_depth_texture_type = 516,
  sym_type_alias_decl = 517,
  sym_type_decl = 518,
  sym_type_decl_without_ident = 519,
  sym_vec_prefix = 520,
  sym_mat_prefix = 521,
  sym_variable_statement = 522,
  sym_variable_decl = 523,
  sym_variable_ident_decl = 524,
  sym_variable_qualifier = 525,
  sym_global_variable_decl = 526,
  sym_global_constant_decl = 527,
  sym_primary_expression = 528,
  sym_callable = 529,
  sym_paren_expression = 530,
  sym_argument_expression_list = 531,
  sym_postfix_expression = 532,
  sym_unary_expression = 533,
  sym_singular_expression = 534,
  sym_lhs_expression = 535,
  sym_core_lhs_expression = 536,
  sym_multiplicative_expression = 537,
  sym_additive_expression = 538,
  sym_relational_expression = 539,
  sym_short_circuit_and_expression = 540,
  sym_short_circuit_or_expression = 541,
  sym_binary_or_expression = 542,
  sym_binary_and_expression = 543,
  sym_binary_xor_expression = 544,
  sym_bitwise_expression = 545,
  sym_expression = 546,
  sym_compound_statement = 547,
  sym_assignment_statement = 548,
  sym_compound_assignment_operator = 549,
  sym_increment_statement = 550,
  sym_decrement_statement = 551,
  sym_if_statement = 552,
  sym_else_statement = 553,
  sym_switch_statement = 554,
  sym_switch_body = 555,
  sym_case_selectors = 556,
  sym_case_compound_statement = 557,
  sym_fallthrough_statement = 558,
  sym_loop_statement = 559,
  sym_for_statement = 560,
  sym_for_header = 561,
  sym_for_init = 562,
  sym_for_update = 563,
  sym_while_statement = 564,
  sym_break_statement = 565,
  sym_break_if_statement = 566,
  sym_continue_statement = 567,
  sym_continuing_statement = 568,
  sym_continuing_compound_statement = 569,
  sym_return_statement = 570,
  sym_func_call_statement = 571,
  sym_static_assert_statement = 572,
  sym_statement = 573,
  sym_function_decl = 574,
  sym_function_header = 575,
  sym_param_list = 576,
  sym_param = 577,
  sym_enable_directive = 578,
  sym_access_mode = 579,
  sym_texel_format = 580,
  sym__comment = 581,
  aux_sym_translation_unit_repeat1 = 582,
  aux_sym_translation_unit_repeat2 = 583,
  aux_sym_attribute_repeat1 = 584,
  aux_sym_struct_body_decl_repeat1 = 585,
  aux_sym_struct_member_repeat1 = 586,
  aux_sym_argument_expression_list_repeat1 = 587,
  aux_sym_lhs_expression_repeat1 = 588,
  aux_sym_compound_statement_repeat1 = 589,
  aux_sym_switch_statement_repeat1 = 590,
  aux_sym_case_selectors_repeat1 = 591,
  aux_sym_param_list_repeat1 = 592,
};

static const char * const ts_symbol_names[] = {
  [ts_builtin_sym_end] = "end",
  [sym_ident] = "ident",
  [aux_sym_int_literal_token1] = "int_literal_token1",
  [aux_sym_int_literal_token2] = "int_literal_token2",
  [aux_sym_int_literal_token3] = "int_literal_token3",
  [aux_sym_decimal_float_literal_token1] = "decimal_float_literal_token1",
  [aux_sym_decimal_float_literal_token2] = "decimal_float_literal_token2",
  [aux_sym_decimal_float_literal_token3] = "decimal_float_literal_token3",
  [aux_sym_decimal_float_literal_token4] = "decimal_float_literal_token4",
  [aux_sym_decimal_float_literal_token5] = "decimal_float_literal_token5",
  [aux_sym_hex_float_literal_token1] = "hex_float_literal_token1",
  [aux_sym_hex_float_literal_token2] = "hex_float_literal_token2",
  [aux_sym_hex_float_literal_token3] = "hex_float_literal_token3",
  [anon_sym_read] = "read",
  [anon_sym_write] = "write",
  [anon_sym_read_write] = "read_write",
  [anon_sym_rgba8unorm] = "rgba8unorm",
  [anon_sym_rgba8snorm] = "rgba8snorm",
  [anon_sym_rgba8uint] = "rgba8uint",
  [anon_sym_rgba8sint] = "rgba8sint",
  [anon_sym_rgba16uint] = "rgba16uint",
  [anon_sym_rgba16sint] = "rgba16sint",
  [anon_sym_rgba16float] = "rgba16float",
  [anon_sym_r32uint] = "r32uint",
  [anon_sym_r32sint] = "r32sint",
  [anon_sym_r32float] = "r32float",
  [anon_sym_rg32uint] = "rg32uint",
  [anon_sym_rg32sint] = "rg32sint",
  [anon_sym_rg32float] = "rg32float",
  [anon_sym_rgba32uint] = "rgba32uint",
  [anon_sym_rgba32sint] = "rgba32sint",
  [anon_sym_rgba32float] = "rgba32float",
  [sym_array] = "array",
  [sym_atomic] = "atomic",
  [sym_bool] = "bool",
  [sym_float32] = "float32",
  [sym_float16] = "float16",
  [sym_int32] = "int32",
  [sym_mat2x2] = "mat2x2",
  [sym_mat2x3] = "mat2x3",
  [sym_mat2x4] = "mat2x4",
  [sym_mat3x2] = "mat3x2",
  [sym_mat3x3] = "mat3x3",
  [sym_mat3x4] = "mat3x4",
  [sym_mat4x2] = "mat4x2",
  [sym_mat4x3] = "mat4x3",
  [sym_mat4x4] = "mat4x4",
  [sym_override] = "override",
  [sym_pointer] = "pointer",
  [sym_sampler] = "sampler",
  [sym_sampler_comparison] = "sampler_comparison",
  [sym_staticAssert] = "staticAssert",
  [sym_struct] = "struct",
  [sym_texture_1d] = "texture_1d",
  [sym_texture_2d] = "texture_2d",
  [sym_texture_2d_array] = "texture_2d_array",
  [sym_texture_3d] = "texture_3d",
  [sym_texture_cube] = "texture_cube",
  [sym_texture_cube_array] = "texture_cube_array",
  [sym_texture_multisampled_2d] = "texture_multisampled_2d",
  [sym_texture_storage_1d] = "texture_storage_1d",
  [sym_texture_storage_2d] = "texture_storage_2d",
  [sym_texture_storage_2d_array] = "texture_storage_2d_array",
  [sym_texture_storage_3d] = "texture_storage_3d",
  [sym_texture_depth_2d] = "texture_depth_2d",
  [sym_texture_depth_2d_array] = "texture_depth_2d_array",
  [sym_texture_depth_cube] = "texture_depth_cube",
  [sym_texture_depth_cube_array] = "texture_depth_cube_array",
  [sym_texture_depth_multisampled_2d] = "texture_depth_multisampled_2d",
  [sym_uint32] = "uint32",
  [sym_vec2] = "vec2",
  [sym_vec3] = "vec3",
  [sym_vec4] = "vec4",
  [sym_bitcast] = "bitcast",
  [sym_break] = "break",
  [sym_case] = "case",
  [sym_const] = "const",
  [sym_continue] = "continue",
  [sym_continuing] = "continuing",
  [sym_default] = "default",
  [sym_discard] = "discard",
  [sym_else] = "else",
  [sym_enable] = "enable",
  [sym_fallthrough] = "fallthrough",
  [sym_false] = "false",
  [sym_fn] = "fn",
  [sym_for] = "for",
  [sym_function] = "function",
  [sym_if] = "if",
  [sym_let] = "let",
  [sym_loop] = "loop",
  [sym_private] = "private",
  [sym_return] = "return",
  [sym_storage] = "storage",
  [sym_switch] = "switch",
  [sym_true] = "true",
  [sym_type] = "type",
  [sym_uniform] = "uniform",
  [sym_var] = "var",
  [sym_while] = "while",
  [sym_workgroup] = "workgroup",
  [sym_and] = "and",
  [sym_and_and] = "and_and",
  [sym_arrow] = "arrow",
  [sym_attr] = "attr",
  [sym_forward_slash] = "forward_slash",
  [sym_bang] = "bang",
  [sym_bracket_left] = "bracket_left",
  [sym_bracket_right] = "bracket_right",
  [sym_brace_left] = "brace_left",
  [sym_brace_right] = "brace_right",
  [sym_colon] = "colon",
  [sym_comma] = "comma",
  [sym_equal] = "equal",
  [sym_equal_equal] = "equal_equal",
  [sym_not_equal] = "not_equal",
  [sym_greater_than] = "greater_than",
  [sym_greater_than_equal] = "greater_than_equal",
  [sym_less_than] = "less_than",
  [sym_less_than_equal] = "less_than_equal",
  [sym_modulo] = "modulo",
  [sym_minus] = "minus",
  [sym_minus_minus] = "minus_minus",
  [sym_period] = "period",
  [sym_plus] = "plus",
  [sym_plus_plus] = "plus_plus",
  [sym_or] = "or",
  [sym_or_or] = "or_or",
  [sym_paren_left] = "paren_left",
  [sym_paren_right] = "paren_right",
  [sym_semicolon] = "semicolon",
  [sym_star] = "star",
  [sym_tilde] = "tilde",
  [sym_underscore] = "underscore",
  [sym_xor] = "xor",
  [sym_plus_equal] = "plus_equal",
  [sym_minus_equal] = "minus_equal",
  [sym_times_equal] = "times_equal",
  [sym_division_equal] = "division_equal",
  [sym_modulo_equal] = "modulo_equal",
  [sym_and_equal] = "and_equal",
  [sym_or_equal] = "or_equal",
  [sym_xor_equal] = "xor_equal",
  [anon_sym_AppendStructuredBuffer] = "AppendStructuredBuffer",
  [anon_sym_BlendState] = "BlendState",
  [anon_sym_Buffer] = "Buffer",
  [anon_sym_ByteAddressBuffer] = "ByteAddressBuffer",
  [anon_sym_CompileShader] = "CompileShader",
  [anon_sym_ComputeShader] = "ComputeShader",
  [anon_sym_ConsumeStructuredBuffer] = "ConsumeStructuredBuffer",
  [anon_sym_DepthStencilState] = "DepthStencilState",
  [anon_sym_DepthStencilView] = "DepthStencilView",
  [anon_sym_DomainShader] = "DomainShader",
  [anon_sym_GeometryShader] = "GeometryShader",
  [anon_sym_Hullshader] = "Hullshader",
  [anon_sym_InputPatch] = "InputPatch",
  [anon_sym_LineStream] = "LineStream",
  [anon_sym_NULL] = "NULL",
  [anon_sym_OutputPatch] = "OutputPatch",
  [anon_sym_PixelShader] = "PixelShader",
  [anon_sym_PointStream] = "PointStream",
  [anon_sym_RWBuffer] = "RWBuffer",
  [anon_sym_RWByteAddressBuffer] = "RWByteAddressBuffer",
  [anon_sym_RWStructuredBuffer] = "RWStructuredBuffer",
  [anon_sym_RWTexture1D] = "RWTexture1D",
  [anon_sym_RWTexture1DArray] = "RWTexture1DArray",
  [anon_sym_RWTexture2D] = "RWTexture2D",
  [anon_sym_RWTexture2DArray] = "RWTexture2DArray",
  [anon_sym_RWTexture3D] = "RWTexture3D",
  [anon_sym_RasterizerState] = "RasterizerState",
  [anon_sym_RenderTargetView] = "RenderTargetView",
  [anon_sym_SamplerComparisonState] = "SamplerComparisonState",
  [anon_sym_SamplerState] = "SamplerState",
  [anon_sym_Self] = "Self",
  [anon_sym_StructuredBuffer] = "StructuredBuffer",
  [anon_sym_Texture1D] = "Texture1D",
  [anon_sym_Texture1DArray] = "Texture1DArray",
  [anon_sym_Texture2D] = "Texture2D",
  [anon_sym_Texture2DArray] = "Texture2DArray",
  [anon_sym_Texture2DMS] = "Texture2DMS",
  [anon_sym_Texture2DMSArray] = "Texture2DMSArray",
  [anon_sym_Texture3D] = "Texture3D",
  [anon_sym_TextureCube] = "TextureCube",
  [anon_sym_TextureCubeArray] = "TextureCubeArray",
  [anon_sym_TriangleStream] = "TriangleStream",
  [anon_sym_VertexShader] = "VertexShader",
  [anon_sym_abstract] = "abstract",
  [anon_sym_active] = "active",
  [anon_sym_alignas] = "alignas",
  [anon_sym_alignof] = "alignof",
  [anon_sym_as] = "as",
  [anon_sym_asm] = "asm",
  [anon_sym_asm_fragment] = "asm_fragment",
  [anon_sym_async] = "async",
  [anon_sym_atomic_uint] = "atomic_uint",
  [anon_sym_attribute] = "attribute",
  [anon_sym_auto] = "auto",
  [anon_sym_await] = "await",
  [anon_sym_become] = "become",
  [anon_sym_bf16] = "bf16",
  [anon_sym_binding_array] = "binding_array",
  [anon_sym_cast] = "cast",
  [anon_sym_catch] = "catch",
  [anon_sym_cbuffer] = "cbuffer",
  [anon_sym_centroid] = "centroid",
  [anon_sym_char] = "char",
  [anon_sym_class] = "class",
  [anon_sym_co_await] = "co_await",
  [anon_sym_co_return] = "co_return",
  [anon_sym_co_yield] = "co_yield",
  [anon_sym_coherent] = "coherent",
  [anon_sym_column_major] = "column_major",
  [anon_sym_common] = "common",
  [anon_sym_compile] = "compile",
  [anon_sym_compile_fragment] = "compile_fragment",
  [anon_sym_concept] = "concept",
  [anon_sym_const_cast] = "const_cast",
  [anon_sym_consteval] = "consteval",
  [anon_sym_constexpr] = "constexpr",
  [anon_sym_constinit] = "constinit",
  [anon_sym_crate] = "crate",
  [anon_sym_debugger] = "debugger",
  [anon_sym_decltype] = "decltype",
  [anon_sym_delete] = "delete",
  [anon_sym_demote] = "demote",
  [anon_sym_demote_to_helper] = "demote_to_helper",
  [anon_sym_do] = "do",
  [anon_sym_dword] = "dword",
  [anon_sym_dynamic_cast] = "dynamic_cast",
  [anon_sym_enum] = "enum",
  [anon_sym_explicit] = "explicit",
  [anon_sym_export] = "export",
  [anon_sym_extends] = "extends",
  [anon_sym_extern] = "extern",
  [anon_sym_external] = "external",
  [anon_sym_f64] = "f64",
  [anon_sym_filter] = "filter",
  [anon_sym_final] = "final",
  [anon_sym_finally] = "finally",
  [anon_sym_fixed] = "fixed",
  [anon_sym_flat] = "flat",
  [anon_sym_friend] = "friend",
  [anon_sym_from] = "from",
  [anon_sym_fvec2] = "fvec2",
  [anon_sym_fvec3] = "fvec3",
  [anon_sym_fvec4] = "fvec4",
  [anon_sym_fxgroup] = "fxgroup",
  [anon_sym_get] = "get",
  [anon_sym_goto] = "goto",
  [anon_sym_groupshared] = "groupshared",
  [anon_sym_handle] = "handle",
  [anon_sym_highp] = "highp",
  [anon_sym_hvec2] = "hvec2",
  [anon_sym_hvec3] = "hvec3",
  [anon_sym_hvec4] = "hvec4",
  [anon_sym_i16] = "i16",
  [anon_sym_i64] = "i64",
  [anon_sym_i8] = "i8",
  [anon_sym_iimage1D] = "iimage1D",
  [anon_sym_iimage1DArray] = "iimage1DArray",
  [anon_sym_iimage2D] = "iimage2D",
  [anon_sym_iimage2DArray] = "iimage2DArray",
  [anon_sym_iimage2DMS] = "iimage2DMS",
  [anon_sym_iimage2DMSArray] = "iimage2DMSArray",
  [anon_sym_iimage2DRect] = "iimage2DRect",
  [anon_sym_iimage3D] = "iimage3D",
  [anon_sym_iimageBuffer] = "iimageBuffer",
  [anon_sym_iimageCube] = "iimageCube",
  [anon_sym_iimageCubeArray] = "iimageCubeArray",
  [anon_sym_image1D] = "image1D",
  [anon_sym_image1DArray] = "image1DArray",
  [anon_sym_image2D] = "image2D",
  [anon_sym_image2DArray] = "image2DArray",
  [anon_sym_image2DMS] = "image2DMS",
  [anon_sym_image2DMSArray] = "image2DMSArray",
  [anon_sym_image2DRect] = "image2DRect",
  [anon_sym_image3D] = "image3D",
  [anon_sym_imageBuffer] = "imageBuffer",
  [anon_sym_imageCube] = "imageCube",
  [anon_sym_imageCubeArray] = "imageCubeArray",
  [anon_sym_impl] = "impl",
  [anon_sym_implements] = "implements",
  [anon_sym_import] = "import",
  [anon_sym_inline] = "inline",
  [anon_sym_inout] = "inout",
  [anon_sym_instanceof] = "instanceof",
  [anon_sym_interface] = "interface",
  [anon_sym_invariant] = "invariant",
  [anon_sym_isampler1D] = "isampler1D",
  [anon_sym_isampler1DArray] = "isampler1DArray",
  [anon_sym_isampler2D] = "isampler2D",
  [anon_sym_isampler2DArray] = "isampler2DArray",
  [anon_sym_isampler2DMS] = "isampler2DMS",
  [anon_sym_isampler2DMSArray] = "isampler2DMSArray",
  [anon_sym_isampler2DRect] = "isampler2DRect",
  [anon_sym_isampler3D] = "isampler3D",
  [anon_sym_isamplerBuffer] = "isamplerBuffer",
  [anon_sym_isamplerCube] = "isamplerCube",
  [anon_sym_isamplerCubeArray] = "isamplerCubeArray",
  [anon_sym_isubpassInput] = "isubpassInput",
  [anon_sym_isubpassInputMS] = "isubpassInputMS",
  [anon_sym_itexture1D] = "itexture1D",
  [anon_sym_itexture1DArray] = "itexture1DArray",
  [anon_sym_itexture2D] = "itexture2D",
  [anon_sym_itexture2DArray] = "itexture2DArray",
  [anon_sym_itexture2DMS] = "itexture2DMS",
  [anon_sym_itexture2DMSArray] = "itexture2DMSArray",
  [anon_sym_itexture2DRect] = "itexture2DRect",
  [anon_sym_itexture3D] = "itexture3D",
  [anon_sym_itextureBuffer] = "itextureBuffer",
  [anon_sym_itextureCube] = "itextureCube",
  [anon_sym_itextureCubeArray] = "itextureCubeArray",
  [anon_sym_layout] = "layout",
  [anon_sym_line] = "line",
  [anon_sym_lineadj] = "lineadj",
  [anon_sym_linear] = "linear",
  [anon_sym_lowp] = "lowp",
  [anon_sym_macro] = "macro",
  [anon_sym_macro_rules] = "macro_rules",
  [anon_sym_mat] = "mat",
  [anon_sym_match] = "match",
  [anon_sym_matrix] = "matrix",
  [anon_sym_mediump] = "mediump",
  [anon_sym_meta] = "meta",
  [anon_sym_mod] = "mod",
  [anon_sym_module] = "module",
  [anon_sym_move] = "move",
  [anon_sym_mut] = "mut",
  [anon_sym_mutable] = "mutable",
  [anon_sym_namespace] = "namespace",
  [anon_sym_new] = "new",
  [anon_sym_nil] = "nil",
  [anon_sym_noexcept] = "noexcept",
  [anon_sym_noinline] = "noinline",
  [anon_sym_nointerpolation] = "nointerpolation",
  [anon_sym_noperspective] = "noperspective",
  [anon_sym_null] = "null",
  [anon_sym_nullptr] = "nullptr",
  [anon_sym_of] = "of",
  [anon_sym_operator] = "operator",
  [anon_sym_package] = "package",
  [anon_sym_packoffset] = "packoffset",
  [anon_sym_partition] = "partition",
  [anon_sym_pass] = "pass",
  [anon_sym_patch] = "patch",
  [anon_sym_pixelfragment] = "pixelfragment",
  [anon_sym_point] = "point",
  [anon_sym_precise] = "precise",
  [anon_sym_precision] = "precision",
  [anon_sym_premerge] = "premerge",
  [anon_sym_priv] = "priv",
  [anon_sym_protected] = "protected",
  [anon_sym_pub] = "pub",
  [anon_sym_public] = "public",
  [anon_sym_readonly] = "readonly",
  [anon_sym_ref] = "ref",
  [anon_sym_regardless] = "regardless",
  [anon_sym_register] = "register",
  [anon_sym_reinterpret_cast] = "reinterpret_cast",
  [anon_sym_requires] = "requires",
  [anon_sym_resource] = "resource",
  [anon_sym_restrict] = "restrict",
  [anon_sym_row_major] = "row_major",
  [anon_sym_samper] = "samper",
  [anon_sym_sample] = "sample",
  [anon_sym_sampler1D] = "sampler1D",
  [anon_sym_sampler1DArray] = "sampler1DArray",
  [anon_sym_sampler1DArrayShadow] = "sampler1DArrayShadow",
  [anon_sym_sampler1DShadow] = "sampler1DShadow",
  [anon_sym_sampler2D] = "sampler2D",
  [anon_sym_sampler2DArray] = "sampler2DArray",
  [anon_sym_sampler2DArrayShadow] = "sampler2DArrayShadow",
  [anon_sym_sampler2DMS] = "sampler2DMS",
  [anon_sym_sampler2DMSArray] = "sampler2DMSArray",
  [anon_sym_sampler2DRect] = "sampler2DRect",
  [anon_sym_sampler2DRectShadow] = "sampler2DRectShadow",
  [anon_sym_sampler2DShadow] = "sampler2DShadow",
  [anon_sym_sampler3D] = "sampler3D",
  [anon_sym_sampler3DRect] = "sampler3DRect",
  [anon_sym_samplerBuffer] = "samplerBuffer",
  [anon_sym_samplerCube] = "samplerCube",
  [anon_sym_samplerCubeArray] = "samplerCubeArray",
  [anon_sym_samplerCubeArrayShadow] = "samplerCubeArrayShadow",
  [anon_sym_samplerCubeShadow] = "samplerCubeShadow",
  [anon_sym_samplerShadow] = "samplerShadow",
  [anon_sym_self] = "self",
  [anon_sym_set] = "set",
  [anon_sym_shared] = "shared",
  [anon_sym_signed] = "signed",
  [anon_sym_sizeof] = "sizeof",
  [anon_sym_smooth] = "smooth",
  [anon_sym_snorm] = "snorm",
  [anon_sym_stateblock] = "stateblock",
  [anon_sym_stateblock_state] = "stateblock_state",
  [anon_sym_static] = "static",
  [anon_sym_static_assert] = "static_assert",
  [anon_sym_static_cast] = "static_cast",
  [anon_sym_std] = "std",
  [anon_sym_string] = "string",
  [anon_sym_subpassInput] = "subpassInput",
  [anon_sym_subpassInputMS] = "subpassInputMS",
  [anon_sym_subroutine] = "subroutine",
  [anon_sym_super] = "super",
  [anon_sym_superp] = "superp",
  [anon_sym_target] = "target",
  [anon_sym_tbuffer] = "tbuffer",
  [anon_sym_technique] = "technique",
  [anon_sym_technique10] = "technique10",
  [anon_sym_technique11] = "technique11",
  [anon_sym_template] = "template",
  [anon_sym_texture1D] = "texture1D",
  [anon_sym_texture1DArray] = "texture1DArray",
  [anon_sym_texture2D] = "texture2D",
  [anon_sym_texture2DArray] = "texture2DArray",
  [anon_sym_texture2DMS] = "texture2DMS",
  [anon_sym_texture2DMSArray] = "texture2DMSArray",
  [anon_sym_texture2DRect] = "texture2DRect",
  [anon_sym_texture3D] = "texture3D",
  [anon_sym_textureBuffer] = "textureBuffer",
  [anon_sym_textureCube] = "textureCube",
  [anon_sym_textureCubeArray] = "textureCubeArray",
  [anon_sym_this] = "this",
  [anon_sym_thread_local] = "thread_local",
  [anon_sym_throw] = "throw",
  [anon_sym_trait] = "trait",
  [anon_sym_triangle] = "triangle",
  [anon_sym_triangleadj] = "triangleadj",
  [anon_sym_try] = "try",
  [anon_sym_typedef] = "typedef",
  [anon_sym_typeid] = "typeid",
  [anon_sym_typename] = "typename",
  [anon_sym_typeof] = "typeof",
  [anon_sym_u16] = "u16",
  [anon_sym_u64] = "u64",
  [anon_sym_u8] = "u8",
  [anon_sym_uimage1D] = "uimage1D",
  [anon_sym_uimage1DArray] = "uimage1DArray",
  [anon_sym_uimage2D] = "uimage2D",
  [anon_sym_uimage2DArray] = "uimage2DArray",
  [anon_sym_uimage2DMS] = "uimage2DMS",
  [anon_sym_uimage2DMSArray] = "uimage2DMSArray",
  [anon_sym_uimage2DRect] = "uimage2DRect",
  [anon_sym_uimage3D] = "uimage3D",
  [anon_sym_uimageBuffer] = "uimageBuffer",
  [anon_sym_uimageCube] = "uimageCube",
  [anon_sym_uimageCubeArray] = "uimageCubeArray",
  [anon_sym_union] = "union",
  [anon_sym_unless] = "unless",
  [anon_sym_unorm] = "unorm",
  [anon_sym_unsafe] = "unsafe",
  [anon_sym_unsigned] = "unsigned",
  [anon_sym_unsized] = "unsized",
  [anon_sym_usampler1D] = "usampler1D",
  [anon_sym_usampler1DArray] = "usampler1DArray",
  [anon_sym_usampler2D] = "usampler2D",
  [anon_sym_usampler2DArray] = "usampler2DArray",
  [anon_sym_usampler2DMS] = "usampler2DMS",
  [anon_sym_usampler2DMSArray] = "usampler2DMSArray",
  [anon_sym_usampler2DRect] = "usampler2DRect",
  [anon_sym_usampler3D] = "usampler3D",
  [anon_sym_usamplerBuffer] = "usamplerBuffer",
  [anon_sym_usamplerCube] = "usamplerCube",
  [anon_sym_usamplerCubeArray] = "usamplerCubeArray",
  [anon_sym_use] = "use",
  [anon_sym_using] = "using",
  [anon_sym_usubpassInput] = "usubpassInput",
  [anon_sym_usubpassInputMS] = "usubpassInputMS",
  [anon_sym_utexture1D] = "utexture1D",
  [anon_sym_utexture1DArray] = "utexture1DArray",
  [anon_sym_utexture2D] = "utexture2D",
  [anon_sym_utexture2DArray] = "utexture2DArray",
  [anon_sym_utexture2DMS] = "utexture2DMS",
  [anon_sym_utexture2DMSArray] = "utexture2DMSArray",
  [anon_sym_utexture2DRect] = "utexture2DRect",
  [anon_sym_utexture3D] = "utexture3D",
  [anon_sym_utextureBuffer] = "utextureBuffer",
  [anon_sym_utextureCube] = "utextureCube",
  [anon_sym_utextureCubeArray] = "utextureCubeArray",
  [anon_sym_varying] = "varying",
  [anon_sym_vec] = "vec",
  [anon_sym_vector] = "vector",
  [anon_sym_vertexfragment] = "vertexfragment",
  [anon_sym_virtual] = "virtual",
  [anon_sym_void] = "void",
  [anon_sym_volatile] = "volatile",
  [anon_sym_wchar_t] = "wchar_t",
  [anon_sym_wgsl] = "wgsl",
  [anon_sym_where] = "where",
  [anon_sym_with] = "with",
  [anon_sym_writeonly] = "writeonly",
  [anon_sym_yield] = "yield",
  [anon_sym_SLASH_SLASH] = "//",
  [aux_sym__comment_token1] = "_comment_token1",
  [sym__blankspace] = "_blankspace",
  [sym__block_comment] = "_block_comment",
  [sym_translation_unit] = "translation_unit",
  [sym_global_directive] = "global_directive",
  [sym_bool_literal] = "bool_literal",
  [sym_int_literal] = "int_literal",
  [sym_float_literal] = "float_literal",
  [sym_decimal_float_literal] = "decimal_float_literal",
  [sym_hex_float_literal] = "hex_float_literal",
  [sym_const_literal] = "const_literal",
  [sym_attribute] = "attribute",
  [sym_literal_or_ident] = "literal_or_ident",
  [sym_array_type_decl] = "array_type_decl",
  [sym_element_count_expression] = "element_count_expression",
  [sym_struct_decl] = "struct_decl",
  [sym_struct_body_decl] = "struct_body_decl",
  [sym_struct_member] = "struct_member",
  [sym_address_space] = "address_space",
  [sym_texture_sampler_types] = "texture_sampler_types",
  [sym_sampler_type] = "sampler_type",
  [sym_sampled_texture_type] = "sampled_texture_type",
  [sym_multisampled_texture_type] = "multisampled_texture_type",
  [sym_storage_texture_type] = "storage_texture_type",
  [sym_depth_texture_type] = "depth_texture_type",
  [sym_type_alias_decl] = "type_alias_decl",
  [sym_type_decl] = "type_decl",
  [sym_type_decl_without_ident] = "type_decl_without_ident",
  [sym_vec_prefix] = "vec_prefix",
  [sym_mat_prefix] = "mat_prefix",
  [sym_variable_statement] = "variable_statement",
  [sym_variable_decl] = "variable_decl",
  [sym_variable_ident_decl] = "variable_ident_decl",
  [sym_variable_qualifier] = "variable_qualifier",
  [sym_global_variable_decl] = "global_variable_decl",
  [sym_global_constant_decl] = "global_constant_decl",
  [sym_primary_expression] = "primary_expression",
  [sym_callable] = "callable",
  [sym_paren_expression] = "paren_expression",
  [sym_argument_expression_list] = "argument_expression_list",
  [sym_postfix_expression] = "postfix_expression",
  [sym_unary_expression] = "unary_expression",
  [sym_singular_expression] = "singular_expression",
  [sym_lhs_expression] = "lhs_expression",
  [sym_core_lhs_expression] = "core_lhs_expression",
  [sym_multiplicative_expression] = "multiplicative_expression",
  [sym_additive_expression] = "additive_expression",
  [sym_relational_expression] = "relational_expression",
  [sym_short_circuit_and_expression] = "short_circuit_and_expression",
  [sym_short_circuit_or_expression] = "short_circuit_or_expression",
  [sym_binary_or_expression] = "binary_or_expression",
  [sym_binary_and_expression] = "binary_and_expression",
  [sym_binary_xor_expression] = "binary_xor_expression",
  [sym_bitwise_expression] = "bitwise_expression",
  [sym_expression] = "expression",
  [sym_compound_statement] = "compound_statement",
  [sym_assignment_statement] = "assignment_statement",
  [sym_compound_assignment_operator] = "compound_assignment_operator",
  [sym_increment_statement] = "increment_statement",
  [sym_decrement_statement] = "decrement_statement",
  [sym_if_statement] = "if_statement",
  [sym_else_statement] = "else_statement",
  [sym_switch_statement] = "switch_statement",
  [sym_switch_body] = "switch_body",
  [sym_case_selectors] = "case_selectors",
  [sym_case_compound_statement] = "case_compound_statement",
  [sym_fallthrough_statement] = "fallthrough_statement",
  [sym_loop_statement] = "loop_statement",
  [sym_for_statement] = "for_statement",
  [sym_for_header] = "for_header",
  [sym_for_init] = "for_init",
  [sym_for_update] = "for_update",
  [sym_while_statement] = "while_statement",
  [sym_break_statement] = "break_statement",
  [sym_break_if_statement] = "break_if_statement",
  [sym_continue_statement] = "continue_statement",
  [sym_continuing_statement] = "continuing_statement",
  [sym_continuing_compound_statement] = "continuing_compound_statement",
  [sym_return_statement] = "return_statement",
  [sym_func_call_statement] = "func_call_statement",
  [sym_static_assert_statement] = "static_assert_statement",
  [sym_statement] = "statement",
  [sym_function_decl] = "function_decl",
  [sym_function_header] = "function_header",
  [sym_param_list] = "param_list",
  [sym_param] = "param",
  [sym_enable_directive] = "enable_directive",
  [sym_access_mode] = "access_mode",
  [sym_texel_format] = "texel_format",
  [sym__comment] = "_comment",
  [aux_sym_translation_unit_repeat1] = "translation_unit_repeat1",
  [aux_sym_translation_unit_repeat2] = "translation_unit_repeat2",
  [aux_sym_attribute_repeat1] = "attribute_repeat1",
  [aux_sym_struct_body_decl_repeat1] = "struct_body_decl_repeat1",
  [aux_sym_struct_member_repeat1] = "struct_member_repeat1",
  [aux_sym_argument_expression_list_repeat1] = "argument_expression_list_repeat1",
  [aux_sym_lhs_expression_repeat1] = "lhs_expression_repeat1",
  [aux_sym_compound_statement_repeat1] = "compound_statement_repeat1",
  [aux_sym_switch_statement_repeat1] = "switch_statement_repeat1",
  [aux_sym_case_selectors_repeat1] = "case_selectors_repeat1",
  [aux_sym_param_list_repeat1] = "param_list_repeat1",
};

static const TSSymbol ts_symbol_map[] = {
  [ts_builtin_sym_end] = ts_builtin_sym_end,
  [sym_ident] = sym_ident,
  [aux_sym_int_literal_token1] = aux_sym_int_literal_token1,
  [aux_sym_int_literal_token2] = aux_sym_int_literal_token2,
  [aux_sym_int_literal_token3] = aux_sym_int_literal_token3,
  [aux_sym_decimal_float_literal_token1] = aux_sym_decimal_float_literal_token1,
  [aux_sym_decimal_float_literal_token2] = aux_sym_decimal_float_literal_token2,
  [aux_sym_decimal_float_literal_token3] = aux_sym_decimal_float_literal_token3,
  [aux_sym_decimal_float_literal_token4] = aux_sym_decimal_float_literal_token4,
  [aux_sym_decimal_float_literal_token5] = aux_sym_decimal_float_literal_token5,
  [aux_sym_hex_float_literal_token1] = aux_sym_hex_float_literal_token1,
  [aux_sym_hex_float_literal_token2] = aux_sym_hex_float_literal_token2,
  [aux_sym_hex_float_literal_token3] = aux_sym_hex_float_literal_token3,
  [anon_sym_read] = anon_sym_read,
  [anon_sym_write] = anon_sym_write,
  [anon_sym_read_write] = anon_sym_read_write,
  [anon_sym_rgba8unorm] = anon_sym_rgba8unorm,
  [anon_sym_rgba8snorm] = anon_sym_rgba8snorm,
  [anon_sym_rgba8uint] = anon_sym_rgba8uint,
  [anon_sym_rgba8sint] = anon_sym_rgba8sint,
  [anon_sym_rgba16uint] = anon_sym_rgba16uint,
  [anon_sym_rgba16sint] = anon_sym_rgba16sint,
  [anon_sym_rgba16float] = anon_sym_rgba16float,
  [anon_sym_r32uint] = anon_sym_r32uint,
  [anon_sym_r32sint] = anon_sym_r32sint,
  [anon_sym_r32float] = anon_sym_r32float,
  [anon_sym_rg32uint] = anon_sym_rg32uint,
  [anon_sym_rg32sint] = anon_sym_rg32sint,
  [anon_sym_rg32float] = anon_sym_rg32float,
  [anon_sym_rgba32uint] = anon_sym_rgba32uint,
  [anon_sym_rgba32sint] = anon_sym_rgba32sint,
  [anon_sym_rgba32float] = anon_sym_rgba32float,
  [sym_array] = sym_array,
  [sym_atomic] = sym_atomic,
  [sym_bool] = sym_bool,
  [sym_float32] = sym_float32,
  [sym_float16] = sym_float16,
  [sym_int32] = sym_int32,
  [sym_mat2x2] = sym_mat2x2,
  [sym_mat2x3] = sym_mat2x3,
  [sym_mat2x4] = sym_mat2x4,
  [sym_mat3x2] = sym_mat3x2,
  [sym_mat3x3] = sym_mat3x3,
  [sym_mat3x4] = sym_mat3x4,
  [sym_mat4x2] = sym_mat4x2,
  [sym_mat4x3] = sym_mat4x3,
  [sym_mat4x4] = sym_mat4x4,
  [sym_override] = sym_override,
  [sym_pointer] = sym_pointer,
  [sym_sampler] = sym_sampler,
  [sym_sampler_comparison] = sym_sampler_comparison,
  [sym_staticAssert] = sym_staticAssert,
  [sym_struct] = sym_struct,
  [sym_texture_1d] = sym_texture_1d,
  [sym_texture_2d] = sym_texture_2d,
  [sym_texture_2d_array] = sym_texture_2d_array,
  [sym_texture_3d] = sym_texture_3d,
  [sym_texture_cube] = sym_texture_cube,
  [sym_texture_cube_array] = sym_texture_cube_array,
  [sym_texture_multisampled_2d] = sym_texture_multisampled_2d,
  [sym_texture_storage_1d] = sym_texture_storage_1d,
  [sym_texture_storage_2d] = sym_texture_storage_2d,
  [sym_texture_storage_2d_array] = sym_texture_storage_2d_array,
  [sym_texture_storage_3d] = sym_texture_storage_3d,
  [sym_texture_depth_2d] = sym_texture_depth_2d,
  [sym_texture_depth_2d_array] = sym_texture_depth_2d_array,
  [sym_texture_depth_cube] = sym_texture_depth_cube,
  [sym_texture_depth_cube_array] = sym_texture_depth_cube_array,
  [sym_texture_depth_multisampled_2d] = sym_texture_depth_multisampled_2d,
  [sym_uint32] = sym_uint32,
  [sym_vec2] = sym_vec2,
  [sym_vec3] = sym_vec3,
  [sym_vec4] = sym_vec4,
  [sym_bitcast] = sym_bitcast,
  [sym_break] = sym_break,
  [sym_case] = sym_case,
  [sym_const] = sym_const,
  [sym_continue] = sym_continue,
  [sym_continuing] = sym_continuing,
  [sym_default] = sym_default,
  [sym_discard] = sym_discard,
  [sym_else] = sym_else,
  [sym_enable] = sym_enable,
  [sym_fallthrough] = sym_fallthrough,
  [sym_false] = sym_false,
  [sym_fn] = sym_fn,
  [sym_for] = sym_for,
  [sym_function] = sym_function,
  [sym_if] = sym_if,
  [sym_let] = sym_let,
  [sym_loop] = sym_loop,
  [sym_private] = sym_private,
  [sym_return] = sym_return,
  [sym_storage] = sym_storage,
  [sym_switch] = sym_switch,
  [sym_true] = sym_true,
  [sym_type] = sym_type,
  [sym_uniform] = sym_uniform,
  [sym_var] = sym_var,
  [sym_while] = sym_while,
  [sym_workgroup] = sym_workgroup,
  [sym_and] = sym_and,
  [sym_and_and] = sym_and_and,
  [sym_arrow] = sym_arrow,
  [sym_attr] = sym_attr,
  [sym_forward_slash] = sym_forward_slash,
  [sym_bang] = sym_bang,
  [sym_bracket_left] = sym_bracket_left,
  [sym_bracket_right] = sym_bracket_right,
  [sym_brace_left] = sym_brace_left,
  [sym_brace_right] = sym_brace_right,
  [sym_colon] = sym_colon,
  [sym_comma] = sym_comma,
  [sym_equal] = sym_equal,
  [sym_equal_equal] = sym_equal_equal,
  [sym_not_equal] = sym_not_equal,
  [sym_greater_than] = sym_greater_than,
  [sym_greater_than_equal] = sym_greater_than_equal,
  [sym_less_than] = sym_less_than,
  [sym_less_than_equal] = sym_less_than_equal,
  [sym_modulo] = sym_modulo,
  [sym_minus] = sym_minus,
  [sym_minus_minus] = sym_minus_minus,
  [sym_period] = sym_period,
  [sym_plus] = sym_plus,
  [sym_plus_plus] = sym_plus_plus,
  [sym_or] = sym_or,
  [sym_or_or] = sym_or_or,
  [sym_paren_left] = sym_paren_left,
  [sym_paren_right] = sym_paren_right,
  [sym_semicolon] = sym_semicolon,
  [sym_star] = sym_star,
  [sym_tilde] = sym_tilde,
  [sym_underscore] = sym_underscore,
  [sym_xor] = sym_xor,
  [sym_plus_equal] = sym_plus_equal,
  [sym_minus_equal] = sym_minus_equal,
  [sym_times_equal] = sym_times_equal,
  [sym_division_equal] = sym_division_equal,
  [sym_modulo_equal] = sym_modulo_equal,
  [sym_and_equal] = sym_and_equal,
  [sym_or_equal] = sym_or_equal,
  [sym_xor_equal] = sym_xor_equal,
  [anon_sym_AppendStructuredBuffer] = anon_sym_AppendStructuredBuffer,
  [anon_sym_BlendState] = anon_sym_BlendState,
  [anon_sym_Buffer] = anon_sym_Buffer,
  [anon_sym_ByteAddressBuffer] = anon_sym_ByteAddressBuffer,
  [anon_sym_CompileShader] = anon_sym_CompileShader,
  [anon_sym_ComputeShader] = anon_sym_ComputeShader,
  [anon_sym_ConsumeStructuredBuffer] = anon_sym_ConsumeStructuredBuffer,
  [anon_sym_DepthStencilState] = anon_sym_DepthStencilState,
  [anon_sym_DepthStencilView] = anon_sym_DepthStencilView,
  [anon_sym_DomainShader] = anon_sym_DomainShader,
  [anon_sym_GeometryShader] = anon_sym_GeometryShader,
  [anon_sym_Hullshader] = anon_sym_Hullshader,
  [anon_sym_InputPatch] = anon_sym_InputPatch,
  [anon_sym_LineStream] = anon_sym_LineStream,
  [anon_sym_NULL] = anon_sym_NULL,
  [anon_sym_OutputPatch] = anon_sym_OutputPatch,
  [anon_sym_PixelShader] = anon_sym_PixelShader,
  [anon_sym_PointStream] = anon_sym_PointStream,
  [anon_sym_RWBuffer] = anon_sym_RWBuffer,
  [anon_sym_RWByteAddressBuffer] = anon_sym_RWByteAddressBuffer,
  [anon_sym_RWStructuredBuffer] = anon_sym_RWStructuredBuffer,
  [anon_sym_RWTexture1D] = anon_sym_RWTexture1D,
  [anon_sym_RWTexture1DArray] = anon_sym_RWTexture1DArray,
  [anon_sym_RWTexture2D] = anon_sym_RWTexture2D,
  [anon_sym_RWTexture2DArray] = anon_sym_RWTexture2DArray,
  [anon_sym_RWTexture3D] = anon_sym_RWTexture3D,
  [anon_sym_RasterizerState] = anon_sym_RasterizerState,
  [anon_sym_RenderTargetView] = anon_sym_RenderTargetView,
  [anon_sym_SamplerComparisonState] = anon_sym_SamplerComparisonState,
  [anon_sym_SamplerState] = anon_sym_SamplerState,
  [anon_sym_Self] = anon_sym_Self,
  [anon_sym_StructuredBuffer] = anon_sym_StructuredBuffer,
  [anon_sym_Texture1D] = anon_sym_Texture1D,
  [anon_sym_Texture1DArray] = anon_sym_Texture1DArray,
  [anon_sym_Texture2D] = anon_sym_Texture2D,
  [anon_sym_Texture2DArray] = anon_sym_Texture2DArray,
  [anon_sym_Texture2DMS] = anon_sym_Texture2DMS,
  [anon_sym_Texture2DMSArray] = anon_sym_Texture2DMSArray,
  [anon_sym_Texture3D] = anon_sym_Texture3D,
  [anon_sym_TextureCube] = anon_sym_TextureCube,
  [anon_sym_TextureCubeArray] = anon_sym_TextureCubeArray,
  [anon_sym_TriangleStream] = anon_sym_TriangleStream,
  [anon_sym_VertexShader] = anon_sym_VertexShader,
  [anon_sym_abstract] = anon_sym_abstract,
  [anon_sym_active] = anon_sym_active,
  [anon_sym_alignas] = anon_sym_alignas,
  [anon_sym_alignof] = anon_sym_alignof,
  [anon_sym_as] = anon_sym_as,
  [anon_sym_asm] = anon_sym_asm,
  [anon_sym_asm_fragment] = anon_sym_asm_fragment,
  [anon_sym_async] = anon_sym_async,
  [anon_sym_atomic_uint] = anon_sym_atomic_uint,
  [anon_sym_attribute] = anon_sym_attribute,
  [anon_sym_auto] = anon_sym_auto,
  [anon_sym_await] = anon_sym_await,
  [anon_sym_become] = anon_sym_become,
  [anon_sym_bf16] = anon_sym_bf16,
  [anon_sym_binding_array] = anon_sym_binding_array,
  [anon_sym_cast] = anon_sym_cast,
  [anon_sym_catch] = anon_sym_catch,
  [anon_sym_cbuffer] = anon_sym_cbuffer,
  [anon_sym_centroid] = anon_sym_centroid,
  [anon_sym_char] = anon_sym_char,
  [anon_sym_class] = anon_sym_class,
  [anon_sym_co_await] = anon_sym_co_await,
  [anon_sym_co_return] = anon_sym_co_return,
  [anon_sym_co_yield] = anon_sym_co_yield,
  [anon_sym_coherent] = anon_sym_coherent,
  [anon_sym_column_major] = anon_sym_column_major,
  [anon_sym_common] = anon_sym_common,
  [anon_sym_compile] = anon_sym_compile,
  [anon_sym_compile_fragment] = anon_sym_compile_fragment,
  [anon_sym_concept] = anon_sym_concept,
  [anon_sym_const_cast] = anon_sym_const_cast,
  [anon_sym_consteval] = anon_sym_consteval,
  [anon_sym_constexpr] = anon_sym_constexpr,
  [anon_sym_constinit] = anon_sym_constinit,
  [anon_sym_crate] = anon_sym_crate,
  [anon_sym_debugger] = anon_sym_debugger,
  [anon_sym_decltype] = anon_sym_decltype,
  [anon_sym_delete] = anon_sym_delete,
  [anon_sym_demote] = anon_sym_demote,
  [anon_sym_demote_to_helper] = anon_sym_demote_to_helper,
  [anon_sym_do] = anon_sym_do,
  [anon_sym_dword] = anon_sym_dword,
  [anon_sym_dynamic_cast] = anon_sym_dynamic_cast,
  [anon_sym_enum] = anon_sym_enum,
  [anon_sym_explicit] = anon_sym_explicit,
  [anon_sym_export] = anon_sym_export,
  [anon_sym_extends] = anon_sym_extends,
  [anon_sym_extern] = anon_sym_extern,
  [anon_sym_external] = anon_sym_external,
  [anon_sym_f64] = anon_sym_f64,
  [anon_sym_filter] = anon_sym_filter,
  [anon_sym_final] = anon_sym_final,
  [anon_sym_finally] = anon_sym_finally,
  [anon_sym_fixed] = anon_sym_fixed,
  [anon_sym_flat] = anon_sym_flat,
  [anon_sym_friend] = anon_sym_friend,
  [anon_sym_from] = anon_sym_from,
  [anon_sym_fvec2] = anon_sym_fvec2,
  [anon_sym_fvec3] = anon_sym_fvec3,
  [anon_sym_fvec4] = anon_sym_fvec4,
  [anon_sym_fxgroup] = anon_sym_fxgroup,
  [anon_sym_get] = anon_sym_get,
  [anon_sym_goto] = anon_sym_goto,
  [anon_sym_groupshared] = anon_sym_groupshared,
  [anon_sym_handle] = anon_sym_handle,
  [anon_sym_highp] = anon_sym_highp,
  [anon_sym_hvec2] = anon_sym_hvec2,
  [anon_sym_hvec3] = anon_sym_hvec3,
  [anon_sym_hvec4] = anon_sym_hvec4,
  [anon_sym_i16] = anon_sym_i16,
  [anon_sym_i64] = anon_sym_i64,
  [anon_sym_i8] = anon_sym_i8,
  [anon_sym_iimage1D] = anon_sym_iimage1D,
  [anon_sym_iimage1DArray] = anon_sym_iimage1DArray,
  [anon_sym_iimage2D] = anon_sym_iimage2D,
  [anon_sym_iimage2DArray] = anon_sym_iimage2DArray,
  [anon_sym_iimage2DMS] = anon_sym_iimage2DMS,
  [anon_sym_iimage2DMSArray] = anon_sym_iimage2DMSArray,
  [anon_sym_iimage2DRect] = anon_sym_iimage2DRect,
  [anon_sym_iimage3D] = anon_sym_iimage3D,
  [anon_sym_iimageBuffer] = anon_sym_iimageBuffer,
  [anon_sym_iimageCube] = anon_sym_iimageCube,
  [anon_sym_iimageCubeArray] = anon_sym_iimageCubeArray,
  [anon_sym_image1D] = anon_sym_image1D,
  [anon_sym_image1DArray] = anon_sym_image1DArray,
  [anon_sym_image2D] = anon_sym_image2D,
  [anon_sym_image2DArray] = anon_sym_image2DArray,
  [anon_sym_image2DMS] = anon_sym_image2DMS,
  [anon_sym_image2DMSArray] = anon_sym_image2DMSArray,
  [anon_sym_image2DRect] = anon_sym_image2DRect,
  [anon_sym_image3D] = anon_sym_image3D,
  [anon_sym_imageBuffer] = anon_sym_imageBuffer,
  [anon_sym_imageCube] = anon_sym_imageCube,
  [anon_sym_imageCubeArray] = anon_sym_imageCubeArray,
  [anon_sym_impl] = anon_sym_impl,
  [anon_sym_implements] = anon_sym_implements,
  [anon_sym_import] = anon_sym_import,
  [anon_sym_inline] = anon_sym_inline,
  [anon_sym_inout] = anon_sym_inout,
  [anon_sym_instanceof] = anon_sym_instanceof,
  [anon_sym_interface] = anon_sym_interface,
  [anon_sym_invariant] = anon_sym_invariant,
  [anon_sym_isampler1D] = anon_sym_isampler1D,
  [anon_sym_isampler1DArray] = anon_sym_isampler1DArray,
  [anon_sym_isampler2D] = anon_sym_isampler2D,
  [anon_sym_isampler2DArray] = anon_sym_isampler2DArray,
  [anon_sym_isampler2DMS] = anon_sym_isampler2DMS,
  [anon_sym_isampler2DMSArray] = anon_sym_isampler2DMSArray,
  [anon_sym_isampler2DRect] = anon_sym_isampler2DRect,
  [anon_sym_isampler3D] = anon_sym_isampler3D,
  [anon_sym_isamplerBuffer] = anon_sym_isamplerBuffer,
  [anon_sym_isamplerCube] = anon_sym_isamplerCube,
  [anon_sym_isamplerCubeArray] = anon_sym_isamplerCubeArray,
  [anon_sym_isubpassInput] = anon_sym_isubpassInput,
  [anon_sym_isubpassInputMS] = anon_sym_isubpassInputMS,
  [anon_sym_itexture1D] = anon_sym_itexture1D,
  [anon_sym_itexture1DArray] = anon_sym_itexture1DArray,
  [anon_sym_itexture2D] = anon_sym_itexture2D,
  [anon_sym_itexture2DArray] = anon_sym_itexture2DArray,
  [anon_sym_itexture2DMS] = anon_sym_itexture2DMS,
  [anon_sym_itexture2DMSArray] = anon_sym_itexture2DMSArray,
  [anon_sym_itexture2DRect] = anon_sym_itexture2DRect,
  [anon_sym_itexture3D] = anon_sym_itexture3D,
  [anon_sym_itextureBuffer] = anon_sym_itextureBuffer,
  [anon_sym_itextureCube] = anon_sym_itextureCube,
  [anon_sym_itextureCubeArray] = anon_sym_itextureCubeArray,
  [anon_sym_layout] = anon_sym_layout,
  [anon_sym_line] = anon_sym_line,
  [anon_sym_lineadj] = anon_sym_lineadj,
  [anon_sym_linear] = anon_sym_linear,
  [anon_sym_lowp] = anon_sym_lowp,
  [anon_sym_macro] = anon_sym_macro,
  [anon_sym_macro_rules] = anon_sym_macro_rules,
  [anon_sym_mat] = anon_sym_mat,
  [anon_sym_match] = anon_sym_match,
  [anon_sym_matrix] = anon_sym_matrix,
  [anon_sym_mediump] = anon_sym_mediump,
  [anon_sym_meta] = anon_sym_meta,
  [anon_sym_mod] = anon_sym_mod,
  [anon_sym_module] = anon_sym_module,
  [anon_sym_move] = anon_sym_move,
  [anon_sym_mut] = anon_sym_mut,
  [anon_sym_mutable] = anon_sym_mutable,
  [anon_sym_namespace] = anon_sym_namespace,
  [anon_sym_new] = anon_sym_new,
  [anon_sym_nil] = anon_sym_nil,
  [anon_sym_noexcept] = anon_sym_noexcept,
  [anon_sym_noinline] = anon_sym_noinline,
  [anon_sym_nointerpolation] = anon_sym_nointerpolation,
  [anon_sym_noperspective] = anon_sym_noperspective,
  [anon_sym_null] = anon_sym_null,
  [anon_sym_nullptr] = anon_sym_nullptr,
  [anon_sym_of] = anon_sym_of,
  [anon_sym_operator] = anon_sym_operator,
  [anon_sym_package] = anon_sym_package,
  [anon_sym_packoffset] = anon_sym_packoffset,
  [anon_sym_partition] = anon_sym_partition,
  [anon_sym_pass] = anon_sym_pass,
  [anon_sym_patch] = anon_sym_patch,
  [anon_sym_pixelfragment] = anon_sym_pixelfragment,
  [anon_sym_point] = anon_sym_point,
  [anon_sym_precise] = anon_sym_precise,
  [anon_sym_precision] = anon_sym_precision,
  [anon_sym_premerge] = anon_sym_premerge,
  [anon_sym_priv] = anon_sym_priv,
  [anon_sym_protected] = anon_sym_protected,
  [anon_sym_pub] = anon_sym_pub,
  [anon_sym_public] = anon_sym_public,
  [anon_sym_readonly] = anon_sym_readonly,
  [anon_sym_ref] = anon_sym_ref,
  [anon_sym_regardless] = anon_sym_regardless,
  [anon_sym_register] = anon_sym_register,
  [anon_sym_reinterpret_cast] = anon_sym_reinterpret_cast,
  [anon_sym_requires] = anon_sym_requires,
  [anon_sym_resource] = anon_sym_resource,
  [anon_sym_restrict] = anon_sym_restrict,
  [anon_sym_row_major] = anon_sym_row_major,
  [anon_sym_samper] = anon_sym_samper,
  [anon_sym_sample] = anon_sym_sample,
  [anon_sym_sampler1D] = anon_sym_sampler1D,
  [anon_sym_sampler1DArray] = anon_sym_sampler1DArray,
  [anon_sym_sampler1DArrayShadow] = anon_sym_sampler1DArrayShadow,
  [anon_sym_sampler1DShadow] = anon_sym_sampler1DShadow,
  [anon_sym_sampler2D] = anon_sym_sampler2D,
  [anon_sym_sampler2DArray] = anon_sym_sampler2DArray,
  [anon_sym_sampler2DArrayShadow] = anon_sym_sampler2DArrayShadow,
  [anon_sym_sampler2DMS] = anon_sym_sampler2DMS,
  [anon_sym_sampler2DMSArray] = anon_sym_sampler2DMSArray,
  [anon_sym_sampler2DRect] = anon_sym_sampler2DRect,
  [anon_sym_sampler2DRectShadow] = anon_sym_sampler2DRectShadow,
  [anon_sym_sampler2DShadow] = anon_sym_sampler2DShadow,
  [anon_sym_sampler3D] = anon_sym_sampler3D,
  [anon_sym_sampler3DRect] = anon_sym_sampler3DRect,
  [anon_sym_samplerBuffer] = anon_sym_samplerBuffer,
  [anon_sym_samplerCube] = anon_sym_samplerCube,
  [anon_sym_samplerCubeArray] = anon_sym_samplerCubeArray,
  [anon_sym_samplerCubeArrayShadow] = anon_sym_samplerCubeArrayShadow,
  [anon_sym_samplerCubeShadow] = anon_sym_samplerCubeShadow,
  [anon_sym_samplerShadow] = anon_sym_samplerShadow,
  [anon_sym_self] = anon_sym_self,
  [anon_sym_set] = anon_sym_set,
  [anon_sym_shared] = anon_sym_shared,
  [anon_sym_signed] = anon_sym_signed,
  [anon_sym_sizeof] = anon_sym_sizeof,
  [anon_sym_smooth] = anon_sym_smooth,
  [anon_sym_snorm] = anon_sym_snorm,
  [anon_sym_stateblock] = anon_sym_stateblock,
  [anon_sym_stateblock_state] = anon_sym_stateblock_state,
  [anon_sym_static] = anon_sym_static,
  [anon_sym_static_assert] = anon_sym_static_assert,
  [anon_sym_static_cast] = anon_sym_static_cast,
  [anon_sym_std] = anon_sym_std,
  [anon_sym_string] = anon_sym_string,
  [anon_sym_subpassInput] = anon_sym_subpassInput,
  [anon_sym_subpassInputMS] = anon_sym_subpassInputMS,
  [anon_sym_subroutine] = anon_sym_subroutine,
  [anon_sym_super] = anon_sym_super,
  [anon_sym_superp] = anon_sym_superp,
  [anon_sym_target] = anon_sym_target,
  [anon_sym_tbuffer] = anon_sym_tbuffer,
  [anon_sym_technique] = anon_sym_technique,
  [anon_sym_technique10] = anon_sym_technique10,
  [anon_sym_technique11] = anon_sym_technique11,
  [anon_sym_template] = anon_sym_template,
  [anon_sym_texture1D] = anon_sym_texture1D,
  [anon_sym_texture1DArray] = anon_sym_texture1DArray,
  [anon_sym_texture2D] = anon_sym_texture2D,
  [anon_sym_texture2DArray] = anon_sym_texture2DArray,
  [anon_sym_texture2DMS] = anon_sym_texture2DMS,
  [anon_sym_texture2DMSArray] = anon_sym_texture2DMSArray,
  [anon_sym_texture2DRect] = anon_sym_texture2DRect,
  [anon_sym_texture3D] = anon_sym_texture3D,
  [anon_sym_textureBuffer] = anon_sym_textureBuffer,
  [anon_sym_textureCube] = anon_sym_textureCube,
  [anon_sym_textureCubeArray] = anon_sym_textureCubeArray,
  [anon_sym_this] = anon_sym_this,
  [anon_sym_thread_local] = anon_sym_thread_local,
  [anon_sym_throw] = anon_sym_throw,
  [anon_sym_trait] = anon_sym_trait,
  [anon_sym_triangle] = anon_sym_triangle,
  [anon_sym_triangleadj] = anon_sym_triangleadj,
  [anon_sym_try] = anon_sym_try,
  [anon_sym_typedef] = anon_sym_typedef,
  [anon_sym_typeid] = anon_sym_typeid,
  [anon_sym_typename] = anon_sym_typename,
  [anon_sym_typeof] = anon_sym_typeof,
  [anon_sym_u16] = anon_sym_u16,
  [anon_sym_u64] = anon_sym_u64,
  [anon_sym_u8] = anon_sym_u8,
  [anon_sym_uimage1D] = anon_sym_uimage1D,
  [anon_sym_uimage1DArray] = anon_sym_uimage1DArray,
  [anon_sym_uimage2D] = anon_sym_uimage2D,
  [anon_sym_uimage2DArray] = anon_sym_uimage2DArray,
  [anon_sym_uimage2DMS] = anon_sym_uimage2DMS,
  [anon_sym_uimage2DMSArray] = anon_sym_uimage2DMSArray,
  [anon_sym_uimage2DRect] = anon_sym_uimage2DRect,
  [anon_sym_uimage3D] = anon_sym_uimage3D,
  [anon_sym_uimageBuffer] = anon_sym_uimageBuffer,
  [anon_sym_uimageCube] = anon_sym_uimageCube,
  [anon_sym_uimageCubeArray] = anon_sym_uimageCubeArray,
  [anon_sym_union] = anon_sym_union,
  [anon_sym_unless] = anon_sym_unless,
  [anon_sym_unorm] = anon_sym_unorm,
  [anon_sym_unsafe] = anon_sym_unsafe,
  [anon_sym_unsigned] = anon_sym_unsigned,
  [anon_sym_unsized] = anon_sym_unsized,
  [anon_sym_usampler1D] = anon_sym_usampler1D,
  [anon_sym_usampler1DArray] = anon_sym_usampler1DArray,
  [anon_sym_usampler2D] = anon_sym_usampler2D,
  [anon_sym_usampler2DArray] = anon_sym_usampler2DArray,
  [anon_sym_usampler2DMS] = anon_sym_usampler2DMS,
  [anon_sym_usampler2DMSArray] = anon_sym_usampler2DMSArray,
  [anon_sym_usampler2DRect] = anon_sym_usampler2DRect,
  [anon_sym_usampler3D] = anon_sym_usampler3D,
  [anon_sym_usamplerBuffer] = anon_sym_usamplerBuffer,
  [anon_sym_usamplerCube] = anon_sym_usamplerCube,
  [anon_sym_usamplerCubeArray] = anon_sym_usamplerCubeArray,
  [anon_sym_use] = anon_sym_use,
  [anon_sym_using] = anon_sym_using,
  [anon_sym_usubpassInput] = anon_sym_usubpassInput,
  [anon_sym_usubpassInputMS] = anon_sym_usubpassInputMS,
  [anon_sym_utexture1D] = anon_sym_utexture1D,
  [anon_sym_utexture1DArray] = anon_sym_utexture1DArray,
  [anon_sym_utexture2D] = anon_sym_utexture2D,
  [anon_sym_utexture2DArray] = anon_sym_utexture2DArray,
  [anon_sym_utexture2DMS] = anon_sym_utexture2DMS,
  [anon_sym_utexture2DMSArray] = anon_sym_utexture2DMSArray,
  [anon_sym_utexture2DRect] = anon_sym_utexture2DRect,
  [anon_sym_utexture3D] = anon_sym_utexture3D,
  [anon_sym_utextureBuffer] = anon_sym_utextureBuffer,
  [anon_sym_utextureCube] = anon_sym_utextureCube,
  [anon_sym_utextureCubeArray] = anon_sym_utextureCubeArray,
  [anon_sym_varying] = anon_sym_varying,
  [anon_sym_vec] = anon_sym_vec,
  [anon_sym_vector] = anon_sym_vector,
  [anon_sym_vertexfragment] = anon_sym_vertexfragment,
  [anon_sym_virtual] = anon_sym_virtual,
  [anon_sym_void] = anon_sym_void,
  [anon_sym_volatile] = anon_sym_volatile,
  [anon_sym_wchar_t] = anon_sym_wchar_t,
  [anon_sym_wgsl] = anon_sym_wgsl,
  [anon_sym_where] = anon_sym_where,
  [anon_sym_with] = anon_sym_with,
  [anon_sym_writeonly] = anon_sym_writeonly,
  [anon_sym_yield] = anon_sym_yield,
  [anon_sym_SLASH_SLASH] = anon_sym_SLASH_SLASH,
  [aux_sym__comment_token1] = aux_sym__comment_token1,
  [sym__blankspace] = sym__blankspace,
  [sym__block_comment] = sym__block_comment,
  [sym_translation_unit] = sym_translation_unit,
  [sym_global_directive] = sym_global_directive,
  [sym_bool_literal] = sym_bool_literal,
  [sym_int_literal] = sym_int_literal,
  [sym_float_literal] = sym_float_literal,
  [sym_decimal_float_literal] = sym_decimal_float_literal,
  [sym_hex_float_literal] = sym_hex_float_literal,
  [sym_const_literal] = sym_const_literal,
  [sym_attribute] = sym_attribute,
  [sym_literal_or_ident] = sym_literal_or_ident,
  [sym_array_type_decl] = sym_array_type_decl,
  [sym_element_count_expression] = sym_element_count_expression,
  [sym_struct_decl] = sym_struct_decl,
  [sym_struct_body_decl] = sym_struct_body_decl,
  [sym_struct_member] = sym_struct_member,
  [sym_address_space] = sym_address_space,
  [sym_texture_sampler_types] = sym_texture_sampler_types,
  [sym_sampler_type] = sym_sampler_type,
  [sym_sampled_texture_type] = sym_sampled_texture_type,
  [sym_multisampled_texture_type] = sym_multisampled_texture_type,
  [sym_storage_texture_type] = sym_storage_texture_type,
  [sym_depth_texture_type] = sym_depth_texture_type,
  [sym_type_alias_decl] = sym_type_alias_decl,
  [sym_type_decl] = sym_type_decl,
  [sym_type_decl_without_ident] = sym_type_decl_without_ident,
  [sym_vec_prefix] = sym_vec_prefix,
  [sym_mat_prefix] = sym_mat_prefix,
  [sym_variable_statement] = sym_variable_statement,
  [sym_variable_decl] = sym_variable_decl,
  [sym_variable_ident_decl] = sym_variable_ident_decl,
  [sym_variable_qualifier] = sym_variable_qualifier,
  [sym_global_variable_decl] = sym_global_variable_decl,
  [sym_global_constant_decl] = sym_global_constant_decl,
  [sym_primary_expression] = sym_primary_expression,
  [sym_callable] = sym_callable,
  [sym_paren_expression] = sym_paren_expression,
  [sym_argument_expression_list] = sym_argument_expression_list,
  [sym_postfix_expression] = sym_postfix_expression,
  [sym_unary_expression] = sym_unary_expression,
  [sym_singular_expression] = sym_singular_expression,
  [sym_lhs_expression] = sym_lhs_expression,
  [sym_core_lhs_expression] = sym_core_lhs_expression,
  [sym_multiplicative_expression] = sym_multiplicative_expression,
  [sym_additive_expression] = sym_additive_expression,
  [sym_relational_expression] = sym_relational_expression,
  [sym_short_circuit_and_expression] = sym_short_circuit_and_expression,
  [sym_short_circuit_or_expression] = sym_short_circuit_or_expression,
  [sym_binary_or_expression] = sym_binary_or_expression,
  [sym_binary_and_expression] = sym_binary_and_expression,
  [sym_binary_xor_expression] = sym_binary_xor_expression,
  [sym_bitwise_expression] = sym_bitwise_expression,
  [sym_expression] = sym_expression,
  [sym_compound_statement] = sym_compound_statement,
  [sym_assignment_statement] = sym_assignment_statement,
  [sym_compound_assignment_operator] = sym_compound_assignment_operator,
  [sym_increment_statement] = sym_increment_statement,
  [sym_decrement_statement] = sym_decrement_statement,
  [sym_if_statement] = sym_if_statement,
  [sym_else_statement] = sym_else_statement,
  [sym_switch_statement] = sym_switch_statement,
  [sym_switch_body] = sym_switch_body,
  [sym_case_selectors] = sym_case_selectors,
  [sym_case_compound_statement] = sym_case_compound_statement,
  [sym_fallthrough_statement] = sym_fallthrough_statement,
  [sym_loop_statement] = sym_loop_statement,
  [sym_for_statement] = sym_for_statement,
  [sym_for_header] = sym_for_header,
  [sym_for_init] = sym_for_init,
  [sym_for_update] = sym_for_update,
  [sym_while_statement] = sym_while_statement,
  [sym_break_statement] = sym_break_statement,
  [sym_break_if_statement] = sym_break_if_statement,
  [sym_continue_statement] = sym_continue_statement,
  [sym_continuing_statement] = sym_continuing_statement,
  [sym_continuing_compound_statement] = sym_continuing_compound_statement,
  [sym_return_statement] = sym_return_statement,
  [sym_func_call_statement] = sym_func_call_statement,
  [sym_static_assert_statement] = sym_static_assert_statement,
  [sym_statement] = sym_statement,
  [sym_function_decl] = sym_function_decl,
  [sym_function_header] = sym_function_header,
  [sym_param_list] = sym_param_list,
  [sym_param] = sym_param,
  [sym_enable_directive] = sym_enable_directive,
  [sym_access_mode] = sym_access_mode,
  [sym_texel_format] = sym_texel_format,
  [sym__comment] = sym__comment,
  [aux_sym_translation_unit_repeat1] = aux_sym_translation_unit_repeat1,
  [aux_sym_translation_unit_repeat2] = aux_sym_translation_unit_repeat2,
  [aux_sym_attribute_repeat1] = aux_sym_attribute_repeat1,
  [aux_sym_struct_body_decl_repeat1] = aux_sym_struct_body_decl_repeat1,
  [aux_sym_struct_member_repeat1] = aux_sym_struct_member_repeat1,
  [aux_sym_argument_expression_list_repeat1] = aux_sym_argument_expression_list_repeat1,
  [aux_sym_lhs_expression_repeat1] = aux_sym_lhs_expression_repeat1,
  [aux_sym_compound_statement_repeat1] = aux_sym_compound_statement_repeat1,
  [aux_sym_switch_statement_repeat1] = aux_sym_switch_statement_repeat1,
  [aux_sym_case_selectors_repeat1] = aux_sym_case_selectors_repeat1,
  [aux_sym_param_list_repeat1] = aux_sym_param_list_repeat1,
};

static const TSSymbolMetadata ts_symbol_metadata[] = {
  [ts_builtin_sym_end] = {
    .visible = false,
    .named = true,
  },
  [sym_ident] = {
    .visible = true,
    .named = true,
  },
  [aux_sym_int_literal_token1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_int_literal_token2] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_int_literal_token3] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_decimal_float_literal_token1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_decimal_float_literal_token2] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_decimal_float_literal_token3] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_decimal_float_literal_token4] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_decimal_float_literal_token5] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_hex_float_literal_token1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_hex_float_literal_token2] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_hex_float_literal_token3] = {
    .visible = false,
    .named = false,
  },
  [anon_sym_read] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_write] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_read_write] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_rgba8unorm] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_rgba8snorm] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_rgba8uint] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_rgba8sint] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_rgba16uint] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_rgba16sint] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_rgba16float] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_r32uint] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_r32sint] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_r32float] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_rg32uint] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_rg32sint] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_rg32float] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_rgba32uint] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_rgba32sint] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_rgba32float] = {
    .visible = true,
    .named = false,
  },
  [sym_array] = {
    .visible = true,
    .named = true,
  },
  [sym_atomic] = {
    .visible = true,
    .named = true,
  },
  [sym_bool] = {
    .visible = true,
    .named = true,
  },
  [sym_float32] = {
    .visible = true,
    .named = true,
  },
  [sym_float16] = {
    .visible = true,
    .named = true,
  },
  [sym_int32] = {
    .visible = true,
    .named = true,
  },
  [sym_mat2x2] = {
    .visible = true,
    .named = true,
  },
  [sym_mat2x3] = {
    .visible = true,
    .named = true,
  },
  [sym_mat2x4] = {
    .visible = true,
    .named = true,
  },
  [sym_mat3x2] = {
    .visible = true,
    .named = true,
  },
  [sym_mat3x3] = {
    .visible = true,
    .named = true,
  },
  [sym_mat3x4] = {
    .visible = true,
    .named = true,
  },
  [sym_mat4x2] = {
    .visible = true,
    .named = true,
  },
  [sym_mat4x3] = {
    .visible = true,
    .named = true,
  },
  [sym_mat4x4] = {
    .visible = true,
    .named = true,
  },
  [sym_override] = {
    .visible = true,
    .named = true,
  },
  [sym_pointer] = {
    .visible = true,
    .named = true,
  },
  [sym_sampler] = {
    .visible = true,
    .named = true,
  },
  [sym_sampler_comparison] = {
    .visible = true,
    .named = true,
  },
  [sym_staticAssert] = {
    .visible = true,
    .named = true,
  },
  [sym_struct] = {
    .visible = true,
    .named = true,
  },
  [sym_texture_1d] = {
    .visible = true,
    .named = true,
  },
  [sym_texture_2d] = {
    .visible = true,
    .named = true,
  },
  [sym_texture_2d_array] = {
    .visible = true,
    .named = true,
  },
  [sym_texture_3d] = {
    .visible = true,
    .named = true,
  },
  [sym_texture_cube] = {
    .visible = true,
    .named = true,
  },
  [sym_texture_cube_array] = {
    .visible = true,
    .named = true,
  },
  [sym_texture_multisampled_2d] = {
    .visible = true,
    .named = true,
  },
  [sym_texture_storage_1d] = {
    .visible = true,
    .named = true,
  },
  [sym_texture_storage_2d] = {
    .visible = true,
    .named = true,
  },
  [sym_texture_storage_2d_array] = {
    .visible = true,
    .named = true,
  },
  [sym_texture_storage_3d] = {
    .visible = true,
    .named = true,
  },
  [sym_texture_depth_2d] = {
    .visible = true,
    .named = true,
  },
  [sym_texture_depth_2d_array] = {
    .visible = true,
    .named = true,
  },
  [sym_texture_depth_cube] = {
    .visible = true,
    .named = true,
  },
  [sym_texture_depth_cube_array] = {
    .visible = true,
    .named = true,
  },
  [sym_texture_depth_multisampled_2d] = {
    .visible = true,
    .named = true,
  },
  [sym_uint32] = {
    .visible = true,
    .named = true,
  },
  [sym_vec2] = {
    .visible = true,
    .named = true,
  },
  [sym_vec3] = {
    .visible = true,
    .named = true,
  },
  [sym_vec4] = {
    .visible = true,
    .named = true,
  },
  [sym_bitcast] = {
    .visible = true,
    .named = true,
  },
  [sym_break] = {
    .visible = true,
    .named = true,
  },
  [sym_case] = {
    .visible = true,
    .named = true,
  },
  [sym_const] = {
    .visible = true,
    .named = true,
  },
  [sym_continue] = {
    .visible = true,
    .named = true,
  },
  [sym_continuing] = {
    .visible = true,
    .named = true,
  },
  [sym_default] = {
    .visible = true,
    .named = true,
  },
  [sym_discard] = {
    .visible = true,
    .named = true,
  },
  [sym_else] = {
    .visible = true,
    .named = true,
  },
  [sym_enable] = {
    .visible = true,
    .named = true,
  },
  [sym_fallthrough] = {
    .visible = true,
    .named = true,
  },
  [sym_false] = {
    .visible = true,
    .named = true,
  },
  [sym_fn] = {
    .visible = true,
    .named = true,
  },
  [sym_for] = {
    .visible = true,
    .named = true,
  },
  [sym_function] = {
    .visible = true,
    .named = true,
  },
  [sym_if] = {
    .visible = true,
    .named = true,
  },
  [sym_let] = {
    .visible = true,
    .named = true,
  },
  [sym_loop] = {
    .visible = true,
    .named = true,
  },
  [sym_private] = {
    .visible = true,
    .named = true,
  },
  [sym_return] = {
    .visible = true,
    .named = true,
  },
  [sym_storage] = {
    .visible = true,
    .named = true,
  },
  [sym_switch] = {
    .visible = true,
    .named = true,
  },
  [sym_true] = {
    .visible = true,
    .named = true,
  },
  [sym_type] = {
    .visible = true,
    .named = true,
  },
  [sym_uniform] = {
    .visible = true,
    .named = true,
  },
  [sym_var] = {
    .visible = true,
    .named = true,
  },
  [sym_while] = {
    .visible = true,
    .named = true,
  },
  [sym_workgroup] = {
    .visible = true,
    .named = true,
  },
  [sym_and] = {
    .visible = true,
    .named = true,
  },
  [sym_and_and] = {
    .visible = true,
    .named = true,
  },
  [sym_arrow] = {
    .visible = true,
    .named = true,
  },
  [sym_attr] = {
    .visible = true,
    .named = true,
  },
  [sym_forward_slash] = {
    .visible = true,
    .named = true,
  },
  [sym_bang] = {
    .visible = true,
    .named = true,
  },
  [sym_bracket_left] = {
    .visible = true,
    .named = true,
  },
  [sym_bracket_right] = {
    .visible = true,
    .named = true,
  },
  [sym_brace_left] = {
    .visible = true,
    .named = true,
  },
  [sym_brace_right] = {
    .visible = true,
    .named = true,
  },
  [sym_colon] = {
    .visible = true,
    .named = true,
  },
  [sym_comma] = {
    .visible = true,
    .named = true,
  },
  [sym_equal] = {
    .visible = true,
    .named = true,
  },
  [sym_equal_equal] = {
    .visible = true,
    .named = true,
  },
  [sym_not_equal] = {
    .visible = true,
    .named = true,
  },
  [sym_greater_than] = {
    .visible = true,
    .named = true,
  },
  [sym_greater_than_equal] = {
    .visible = true,
    .named = true,
  },
  [sym_less_than] = {
    .visible = true,
    .named = true,
  },
  [sym_less_than_equal] = {
    .visible = true,
    .named = true,
  },
  [sym_modulo] = {
    .visible = true,
    .named = true,
  },
  [sym_minus] = {
    .visible = true,
    .named = true,
  },
  [sym_minus_minus] = {
    .visible = true,
    .named = true,
  },
  [sym_period] = {
    .visible = true,
    .named = true,
  },
  [sym_plus] = {
    .visible = true,
    .named = true,
  },
  [sym_plus_plus] = {
    .visible = true,
    .named = true,
  },
  [sym_or] = {
    .visible = true,
    .named = true,
  },
  [sym_or_or] = {
    .visible = true,
    .named = true,
  },
  [sym_paren_left] = {
    .visible = true,
    .named = true,
  },
  [sym_paren_right] = {
    .visible = true,
    .named = true,
  },
  [sym_semicolon] = {
    .visible = true,
    .named = true,
  },
  [sym_star] = {
    .visible = true,
    .named = true,
  },
  [sym_tilde] = {
    .visible = true,
    .named = true,
  },
  [sym_underscore] = {
    .visible = true,
    .named = true,
  },
  [sym_xor] = {
    .visible = true,
    .named = true,
  },
  [sym_plus_equal] = {
    .visible = true,
    .named = true,
  },
  [sym_minus_equal] = {
    .visible = true,
    .named = true,
  },
  [sym_times_equal] = {
    .visible = true,
    .named = true,
  },
  [sym_division_equal] = {
    .visible = true,
    .named = true,
  },
  [sym_modulo_equal] = {
    .visible = true,
    .named = true,
  },
  [sym_and_equal] = {
    .visible = true,
    .named = true,
  },
  [sym_or_equal] = {
    .visible = true,
    .named = true,
  },
  [sym_xor_equal] = {
    .visible = true,
    .named = true,
  },
  [anon_sym_AppendStructuredBuffer] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_BlendState] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_Buffer] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_ByteAddressBuffer] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_CompileShader] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_ComputeShader] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_ConsumeStructuredBuffer] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_DepthStencilState] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_DepthStencilView] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_DomainShader] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_GeometryShader] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_Hullshader] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_InputPatch] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_LineStream] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_NULL] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_OutputPatch] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_PixelShader] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_PointStream] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_RWBuffer] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_RWByteAddressBuffer] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_RWStructuredBuffer] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_RWTexture1D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_RWTexture1DArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_RWTexture2D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_RWTexture2DArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_RWTexture3D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_RasterizerState] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_RenderTargetView] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_SamplerComparisonState] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_SamplerState] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_Self] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_StructuredBuffer] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_Texture1D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_Texture1DArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_Texture2D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_Texture2DArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_Texture2DMS] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_Texture2DMSArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_Texture3D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_TextureCube] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_TextureCubeArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_TriangleStream] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_VertexShader] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_abstract] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_active] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_alignas] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_alignof] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_as] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_asm] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_asm_fragment] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_async] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_atomic_uint] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_attribute] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_auto] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_await] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_become] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_bf16] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_binding_array] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_cast] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_catch] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_cbuffer] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_centroid] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_char] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_class] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_co_await] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_co_return] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_co_yield] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_coherent] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_column_major] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_common] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_compile] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_compile_fragment] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_concept] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_const_cast] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_consteval] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_constexpr] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_constinit] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_crate] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_debugger] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_decltype] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_delete] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_demote] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_demote_to_helper] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_do] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_dword] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_dynamic_cast] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_enum] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_explicit] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_export] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_extends] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_extern] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_external] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_f64] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_filter] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_final] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_finally] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_fixed] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_flat] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_friend] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_from] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_fvec2] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_fvec3] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_fvec4] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_fxgroup] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_get] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_goto] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_groupshared] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_handle] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_highp] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_hvec2] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_hvec3] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_hvec4] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_i16] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_i64] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_i8] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_iimage1D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_iimage1DArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_iimage2D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_iimage2DArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_iimage2DMS] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_iimage2DMSArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_iimage2DRect] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_iimage3D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_iimageBuffer] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_iimageCube] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_iimageCubeArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_image1D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_image1DArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_image2D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_image2DArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_image2DMS] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_image2DMSArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_image2DRect] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_image3D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_imageBuffer] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_imageCube] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_imageCubeArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_impl] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_implements] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_import] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_inline] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_inout] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_instanceof] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_interface] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_invariant] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_isampler1D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_isampler1DArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_isampler2D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_isampler2DArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_isampler2DMS] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_isampler2DMSArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_isampler2DRect] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_isampler3D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_isamplerBuffer] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_isamplerCube] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_isamplerCubeArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_isubpassInput] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_isubpassInputMS] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_itexture1D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_itexture1DArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_itexture2D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_itexture2DArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_itexture2DMS] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_itexture2DMSArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_itexture2DRect] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_itexture3D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_itextureBuffer] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_itextureCube] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_itextureCubeArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_layout] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_line] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_lineadj] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_linear] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_lowp] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_macro] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_macro_rules] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_mat] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_match] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_matrix] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_mediump] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_meta] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_mod] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_module] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_move] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_mut] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_mutable] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_namespace] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_new] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_nil] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_noexcept] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_noinline] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_nointerpolation] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_noperspective] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_null] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_nullptr] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_of] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_operator] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_package] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_packoffset] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_partition] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_pass] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_patch] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_pixelfragment] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_point] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_precise] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_precision] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_premerge] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_priv] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_protected] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_pub] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_public] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_readonly] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_ref] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_regardless] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_register] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_reinterpret_cast] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_requires] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_resource] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_restrict] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_row_major] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_samper] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_sample] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_sampler1D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_sampler1DArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_sampler1DArrayShadow] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_sampler1DShadow] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_sampler2D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_sampler2DArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_sampler2DArrayShadow] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_sampler2DMS] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_sampler2DMSArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_sampler2DRect] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_sampler2DRectShadow] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_sampler2DShadow] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_sampler3D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_sampler3DRect] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_samplerBuffer] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_samplerCube] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_samplerCubeArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_samplerCubeArrayShadow] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_samplerCubeShadow] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_samplerShadow] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_self] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_set] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_shared] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_signed] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_sizeof] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_smooth] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_snorm] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_stateblock] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_stateblock_state] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_static] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_static_assert] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_static_cast] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_std] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_string] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_subpassInput] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_subpassInputMS] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_subroutine] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_super] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_superp] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_target] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_tbuffer] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_technique] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_technique10] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_technique11] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_template] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_texture1D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_texture1DArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_texture2D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_texture2DArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_texture2DMS] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_texture2DMSArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_texture2DRect] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_texture3D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_textureBuffer] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_textureCube] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_textureCubeArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_this] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_thread_local] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_throw] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_trait] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_triangle] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_triangleadj] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_try] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_typedef] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_typeid] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_typename] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_typeof] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_u16] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_u64] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_u8] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_uimage1D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_uimage1DArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_uimage2D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_uimage2DArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_uimage2DMS] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_uimage2DMSArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_uimage2DRect] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_uimage3D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_uimageBuffer] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_uimageCube] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_uimageCubeArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_union] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_unless] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_unorm] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_unsafe] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_unsigned] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_unsized] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_usampler1D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_usampler1DArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_usampler2D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_usampler2DArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_usampler2DMS] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_usampler2DMSArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_usampler2DRect] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_usampler3D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_usamplerBuffer] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_usamplerCube] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_usamplerCubeArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_use] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_using] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_usubpassInput] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_usubpassInputMS] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_utexture1D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_utexture1DArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_utexture2D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_utexture2DArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_utexture2DMS] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_utexture2DMSArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_utexture2DRect] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_utexture3D] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_utextureBuffer] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_utextureCube] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_utextureCubeArray] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_varying] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_vec] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_vector] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_vertexfragment] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_virtual] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_void] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_volatile] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_wchar_t] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_wgsl] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_where] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_with] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_writeonly] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_yield] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_SLASH_SLASH] = {
    .visible = true,
    .named = false,
  },
  [aux_sym__comment_token1] = {
    .visible = false,
    .named = false,
  },
  [sym__blankspace] = {
    .visible = false,
    .named = true,
  },
  [sym__block_comment] = {
    .visible = false,
    .named = true,
  },
  [sym_translation_unit] = {
    .visible = true,
    .named = true,
  },
  [sym_global_directive] = {
    .visible = true,
    .named = true,
  },
  [sym_bool_literal] = {
    .visible = true,
    .named = true,
  },
  [sym_int_literal] = {
    .visible = true,
    .named = true,
  },
  [sym_float_literal] = {
    .visible = true,
    .named = true,
  },
  [sym_decimal_float_literal] = {
    .visible = true,
    .named = true,
  },
  [sym_hex_float_literal] = {
    .visible = true,
    .named = true,
  },
  [sym_const_literal] = {
    .visible = true,
    .named = true,
  },
  [sym_attribute] = {
    .visible = true,
    .named = true,
  },
  [sym_literal_or_ident] = {
    .visible = true,
    .named = true,
  },
  [sym_array_type_decl] = {
    .visible = true,
    .named = true,
  },
  [sym_element_count_expression] = {
    .visible = true,
    .named = true,
  },
  [sym_struct_decl] = {
    .visible = true,
    .named = true,
  },
  [sym_struct_body_decl] = {
    .visible = true,
    .named = true,
  },
  [sym_struct_member] = {
    .visible = true,
    .named = true,
  },
  [sym_address_space] = {
    .visible = true,
    .named = true,
  },
  [sym_texture_sampler_types] = {
    .visible = true,
    .named = true,
  },
  [sym_sampler_type] = {
    .visible = true,
    .named = true,
  },
  [sym_sampled_texture_type] = {
    .visible = true,
    .named = true,
  },
  [sym_multisampled_texture_type] = {
    .visible = true,
    .named = true,
  },
  [sym_storage_texture_type] = {
    .visible = true,
    .named = true,
  },
  [sym_depth_texture_type] = {
    .visible = true,
    .named = true,
  },
  [sym_type_alias_decl] = {
    .visible = true,
    .named = true,
  },
  [sym_type_decl] = {
    .visible = true,
    .named = true,
  },
  [sym_type_decl_without_ident] = {
    .visible = true,
    .named = true,
  },
  [sym_vec_prefix] = {
    .visible = true,
    .named = true,
  },
  [sym_mat_prefix] = {
    .visible = true,
    .named = true,
  },
  [sym_variable_statement] = {
    .visible = true,
    .named = true,
  },
  [sym_variable_decl] = {
    .visible = true,
    .named = true,
  },
  [sym_variable_ident_decl] = {
    .visible = true,
    .named = true,
  },
  [sym_variable_qualifier] = {
    .visible = true,
    .named = true,
  },
  [sym_global_variable_decl] = {
    .visible = true,
    .named = true,
  },
  [sym_global_constant_decl] = {
    .visible = true,
    .named = true,
  },
  [sym_primary_expression] = {
    .visible = true,
    .named = true,
  },
  [sym_callable] = {
    .visible = true,
    .named = true,
  },
  [sym_paren_expression] = {
    .visible = true,
    .named = true,
  },
  [sym_argument_expression_list] = {
    .visible = true,
    .named = true,
  },
  [sym_postfix_expression] = {
    .visible = true,
    .named = true,
  },
  [sym_unary_expression] = {
    .visible = true,
    .named = true,
  },
  [sym_singular_expression] = {
    .visible = true,
    .named = true,
  },
  [sym_lhs_expression] = {
    .visible = true,
    .named = true,
  },
  [sym_core_lhs_expression] = {
    .visible = true,
    .named = true,
  },
  [sym_multiplicative_expression] = {
    .visible = true,
    .named = true,
  },
  [sym_additive_expression] = {
    .visible = true,
    .named = true,
  },
  [sym_relational_expression] = {
    .visible = true,
    .named = true,
  },
  [sym_short_circuit_and_expression] = {
    .visible = true,
    .named = true,
  },
  [sym_short_circuit_or_expression] = {
    .visible = true,
    .named = true,
  },
  [sym_binary_or_expression] = {
    .visible = true,
    .named = true,
  },
  [sym_binary_and_expression] = {
    .visible = true,
    .named = true,
  },
  [sym_binary_xor_expression] = {
    .visible = true,
    .named = true,
  },
  [sym_bitwise_expression] = {
    .visible = true,
    .named = true,
  },
  [sym_expression] = {
    .visible = true,
    .named = true,
  },
  [sym_compound_statement] = {
    .visible = true,
    .named = true,
  },
  [sym_assignment_statement] = {
    .visible = true,
    .named = true,
  },
  [sym_compound_assignment_operator] = {
    .visible = true,
    .named = true,
  },
  [sym_increment_statement] = {
    .visible = true,
    .named = true,
  },
  [sym_decrement_statement] = {
    .visible = true,
    .named = true,
  },
  [sym_if_statement] = {
    .visible = true,
    .named = true,
  },
  [sym_else_statement] = {
    .visible = true,
    .named = true,
  },
  [sym_switch_statement] = {
    .visible = true,
    .named = true,
  },
  [sym_switch_body] = {
    .visible = true,
    .named = true,
  },
  [sym_case_selectors] = {
    .visible = true,
    .named = true,
  },
  [sym_case_compound_statement] = {
    .visible = true,
    .named = true,
  },
  [sym_fallthrough_statement] = {
    .visible = true,
    .named = true,
  },
  [sym_loop_statement] = {
    .visible = true,
    .named = true,
  },
  [sym_for_statement] = {
    .visible = true,
    .named = true,
  },
  [sym_for_header] = {
    .visible = true,
    .named = true,
  },
  [sym_for_init] = {
    .visible = true,
    .named = true,
  },
  [sym_for_update] = {
    .visible = true,
    .named = true,
  },
  [sym_while_statement] = {
    .visible = true,
    .named = true,
  },
  [sym_break_statement] = {
    .visible = true,
    .named = true,
  },
  [sym_break_if_statement] = {
    .visible = true,
    .named = true,
  },
  [sym_continue_statement] = {
    .visible = true,
    .named = true,
  },
  [sym_continuing_statement] = {
    .visible = true,
    .named = true,
  },
  [sym_continuing_compound_statement] = {
    .visible = true,
    .named = true,
  },
  [sym_return_statement] = {
    .visible = true,
    .named = true,
  },
  [sym_func_call_statement] = {
    .visible = true,
    .named = true,
  },
  [sym_static_assert_statement] = {
    .visible = true,
    .named = true,
  },
  [sym_statement] = {
    .visible = true,
    .named = true,
  },
  [sym_function_decl] = {
    .visible = true,
    .named = true,
  },
  [sym_function_header] = {
    .visible = true,
    .named = true,
  },
  [sym_param_list] = {
    .visible = true,
    .named = true,
  },
  [sym_param] = {
    .visible = true,
    .named = true,
  },
  [sym_enable_directive] = {
    .visible = true,
    .named = true,
  },
  [sym_access_mode] = {
    .visible = true,
    .named = true,
  },
  [sym_texel_format] = {
    .visible = true,
    .named = true,
  },
  [sym__comment] = {
    .visible = false,
    .named = true,
  },
  [aux_sym_translation_unit_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_translation_unit_repeat2] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_attribute_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_struct_body_decl_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_struct_member_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_argument_expression_list_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_lhs_expression_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_compound_statement_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_switch_statement_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_case_selectors_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_param_list_repeat1] = {
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

static inline bool sym_ident_character_set_1(int32_t c) {
  return (c < 43514
    ? (c < 4193
      ? (c < 2707
        ? (c < 1994
          ? (c < 931
            ? (c < 748
              ? (c < 192
                ? (c < 170
                  ? (c < 'a'
                    ? (c >= 'A' && c <= 'Z')
                    : c <= 'z')
                  : (c <= 170 || (c < 186
                    ? c == 181
                    : c <= 186)))
                : (c <= 214 || (c < 710
                  ? (c < 248
                    ? (c >= 216 && c <= 246)
                    : c <= 705)
                  : (c <= 721 || (c >= 736 && c <= 740)))))
              : (c <= 748 || (c < 895
                ? (c < 886
                  ? (c < 880
                    ? c == 750
                    : c <= 884)
                  : (c <= 887 || (c >= 891 && c <= 893)))
                : (c <= 895 || (c < 908
                  ? (c < 904
                    ? c == 902
                    : c <= 906)
                  : (c <= 908 || (c >= 910 && c <= 929)))))))
            : (c <= 1013 || (c < 1649
              ? (c < 1376
                ? (c < 1329
                  ? (c < 1162
                    ? (c >= 1015 && c <= 1153)
                    : c <= 1327)
                  : (c <= 1366 || c == 1369))
                : (c <= 1416 || (c < 1568
                  ? (c < 1519
                    ? (c >= 1488 && c <= 1514)
                    : c <= 1522)
                  : (c <= 1610 || (c >= 1646 && c <= 1647)))))
              : (c <= 1747 || (c < 1791
                ? (c < 1774
                  ? (c < 1765
                    ? c == 1749
                    : c <= 1766)
                  : (c <= 1775 || (c >= 1786 && c <= 1788)))
                : (c <= 1791 || (c < 1869
                  ? (c < 1810
                    ? c == 1808
                    : c <= 1839)
                  : (c <= 1957 || c == 1969))))))))
          : (c <= 2026 || (c < 2482
            ? (c < 2208
              ? (c < 2088
                ? (c < 2048
                  ? (c < 2042
                    ? (c >= 2036 && c <= 2037)
                    : c <= 2042)
                  : (c <= 2069 || (c < 2084
                    ? c == 2074
                    : c <= 2084)))
                : (c <= 2088 || (c < 2160
                  ? (c < 2144
                    ? (c >= 2112 && c <= 2136)
                    : c <= 2154)
                  : (c <= 2183 || (c >= 2185 && c <= 2190)))))
              : (c <= 2249 || (c < 2417
                ? (c < 2384
                  ? (c < 2365
                    ? (c >= 2308 && c <= 2361)
                    : c <= 2365)
                  : (c <= 2384 || (c >= 2392 && c <= 2401)))
                : (c <= 2432 || (c < 2451
                  ? (c < 2447
                    ? (c >= 2437 && c <= 2444)
                    : c <= 2448)
                  : (c <= 2472 || (c >= 2474 && c <= 2480)))))))
            : (c <= 2482 || (c < 2579
              ? (c < 2527
                ? (c < 2510
                  ? (c < 2493
                    ? (c >= 2486 && c <= 2489)
                    : c <= 2493)
                  : (c <= 2510 || (c >= 2524 && c <= 2525)))
                : (c <= 2529 || (c < 2565
                  ? (c < 2556
                    ? (c >= 2544 && c <= 2545)
                    : c <= 2556)
                  : (c <= 2570 || (c >= 2575 && c <= 2576)))))
              : (c <= 2600 || (c < 2649
                ? (c < 2613
                  ? (c < 2610
                    ? (c >= 2602 && c <= 2608)
                    : c <= 2611)
                  : (c <= 2614 || (c >= 2616 && c <= 2617)))
                : (c <= 2652 || (c < 2693
                  ? (c < 2674
                    ? c == 2654
                    : c <= 2676)
                  : (c <= 2701 || (c >= 2703 && c <= 2705)))))))))))
        : (c <= 2728 || (c < 3242
          ? (c < 2962
            ? (c < 2858
              ? (c < 2784
                ? (c < 2741
                  ? (c < 2738
                    ? (c >= 2730 && c <= 2736)
                    : c <= 2739)
                  : (c <= 2745 || (c < 2768
                    ? c == 2749
                    : c <= 2768)))
                : (c <= 2785 || (c < 2831
                  ? (c < 2821
                    ? c == 2809
                    : c <= 2828)
                  : (c <= 2832 || (c >= 2835 && c <= 2856)))))
              : (c <= 2864 || (c < 2911
                ? (c < 2877
                  ? (c < 2869
                    ? (c >= 2866 && c <= 2867)
                    : c <= 2873)
                  : (c <= 2877 || (c >= 2908 && c <= 2909)))
                : (c <= 2913 || (c < 2949
                  ? (c < 2947
                    ? c == 2929
                    : c <= 2947)
                  : (c <= 2954 || (c >= 2958 && c <= 2960)))))))
            : (c <= 2965 || (c < 3090
              ? (c < 2984
                ? (c < 2974
                  ? (c < 2972
                    ? (c >= 2969 && c <= 2970)
                    : c <= 2972)
                  : (c <= 2975 || (c >= 2979 && c <= 2980)))
                : (c <= 2986 || (c < 3077
                  ? (c < 3024
                    ? (c >= 2990 && c <= 3001)
                    : c <= 3024)
                  : (c <= 3084 || (c >= 3086 && c <= 3088)))))
              : (c <= 3112 || (c < 3168
                ? (c < 3160
                  ? (c < 3133
                    ? (c >= 3114 && c <= 3129)
                    : c <= 3133)
                  : (c <= 3162 || c == 3165))
                : (c <= 3169 || (c < 3214
                  ? (c < 3205
                    ? c == 3200
                    : c <= 3212)
                  : (c <= 3216 || (c >= 3218 && c <= 3240)))))))))
          : (c <= 3251 || (c < 3648
            ? (c < 3412
              ? (c < 3332
                ? (c < 3293
                  ? (c < 3261
                    ? (c >= 3253 && c <= 3257)
                    : c <= 3261)
                  : (c <= 3294 || (c < 3313
                    ? (c >= 3296 && c <= 3297)
                    : c <= 3314)))
                : (c <= 3340 || (c < 3389
                  ? (c < 3346
                    ? (c >= 3342 && c <= 3344)
                    : c <= 3386)
                  : (c <= 3389 || c == 3406))))
              : (c <= 3414 || (c < 3507
                ? (c < 3461
                  ? (c < 3450
                    ? (c >= 3423 && c <= 3425)
                    : c <= 3455)
                  : (c <= 3478 || (c >= 3482 && c <= 3505)))
                : (c <= 3515 || (c < 3585
                  ? (c < 3520
                    ? c == 3517
                    : c <= 3526)
                  : (c <= 3632 || c == 3634))))))
            : (c <= 3654 || (c < 3782
              ? (c < 3749
                ? (c < 3718
                  ? (c < 3716
                    ? (c >= 3713 && c <= 3714)
                    : c <= 3716)
                  : (c <= 3722 || (c >= 3724 && c <= 3747)))
                : (c <= 3749 || (c < 3773
                  ? (c < 3762
                    ? (c >= 3751 && c <= 3760)
                    : c <= 3762)
                  : (c <= 3773 || (c >= 3776 && c <= 3780)))))
              : (c <= 3782 || (c < 3976
                ? (c < 3904
                  ? (c < 3840
                    ? (c >= 3804 && c <= 3807)
                    : c <= 3840)
                  : (c <= 3911 || (c >= 3913 && c <= 3948)))
                : (c <= 3980 || (c < 4176
                  ? (c < 4159
                    ? (c >= 4096 && c <= 4138)
                    : c <= 4159)
                  : (c <= 4181 || (c >= 4186 && c <= 4189)))))))))))))
      : (c <= 4193 || (c < 8134
        ? (c < 6176
          ? (c < 4808
            ? (c < 4688
              ? (c < 4295
                ? (c < 4213
                  ? (c < 4206
                    ? (c >= 4197 && c <= 4198)
                    : c <= 4208)
                  : (c <= 4225 || (c < 4256
                    ? c == 4238
                    : c <= 4293)))
                : (c <= 4295 || (c < 4348
                  ? (c < 4304
                    ? c == 4301
                    : c <= 4346)
                  : (c <= 4680 || (c >= 4682 && c <= 4685)))))
              : (c <= 4694 || (c < 4752
                ? (c < 4704
                  ? (c < 4698
                    ? c == 4696
                    : c <= 4701)
                  : (c <= 4744 || (c >= 4746 && c <= 4749)))
                : (c <= 4784 || (c < 4800
                  ? (c < 4792
                    ? (c >= 4786 && c <= 4789)
                    : c <= 4798)
                  : (c <= 4800 || (c >= 4802 && c <= 4805)))))))
            : (c <= 4822 || (c < 5792
              ? (c < 5024
                ? (c < 4888
                  ? (c < 4882
                    ? (c >= 4824 && c <= 4880)
                    : c <= 4885)
                  : (c <= 4954 || (c >= 4992 && c <= 5007)))
                : (c <= 5109 || (c < 5743
                  ? (c < 5121
                    ? (c >= 5112 && c <= 5117)
                    : c <= 5740)
                  : (c <= 5759 || (c >= 5761 && c <= 5786)))))
              : (c <= 5866 || (c < 5984
                ? (c < 5919
                  ? (c < 5888
                    ? (c >= 5870 && c <= 5880)
                    : c <= 5905)
                  : (c <= 5937 || (c >= 5952 && c <= 5969)))
                : (c <= 5996 || (c < 6103
                  ? (c < 6016
                    ? (c >= 5998 && c <= 6000)
                    : c <= 6067)
                  : (c <= 6103 || c == 6108))))))))
          : (c <= 6264 || (c < 7312
            ? (c < 6823
              ? (c < 6512
                ? (c < 6320
                  ? (c < 6314
                    ? (c >= 6272 && c <= 6312)
                    : c <= 6314)
                  : (c <= 6389 || (c < 6480
                    ? (c >= 6400 && c <= 6430)
                    : c <= 6509)))
                : (c <= 6516 || (c < 6656
                  ? (c < 6576
                    ? (c >= 6528 && c <= 6571)
                    : c <= 6601)
                  : (c <= 6678 || (c >= 6688 && c <= 6740)))))
              : (c <= 6823 || (c < 7098
                ? (c < 7043
                  ? (c < 6981
                    ? (c >= 6917 && c <= 6963)
                    : c <= 6988)
                  : (c <= 7072 || (c >= 7086 && c <= 7087)))
                : (c <= 7141 || (c < 7258
                  ? (c < 7245
                    ? (c >= 7168 && c <= 7203)
                    : c <= 7247)
                  : (c <= 7293 || (c >= 7296 && c <= 7304)))))))
            : (c <= 7354 || (c < 8008
              ? (c < 7418
                ? (c < 7406
                  ? (c < 7401
                    ? (c >= 7357 && c <= 7359)
                    : c <= 7404)
                  : (c <= 7411 || (c >= 7413 && c <= 7414)))
                : (c <= 7418 || (c < 7960
                  ? (c < 7680
                    ? (c >= 7424 && c <= 7615)
                    : c <= 7957)
                  : (c <= 7965 || (c >= 7968 && c <= 8005)))))
              : (c <= 8013 || (c < 8031
                ? (c < 8027
                  ? (c < 8025
                    ? (c >= 8016 && c <= 8023)
                    : c <= 8025)
                  : (c <= 8027 || c == 8029))
                : (c <= 8061 || (c < 8126
                  ? (c < 8118
                    ? (c >= 8064 && c <= 8116)
                    : c <= 8124)
                  : (c <= 8126 || (c >= 8130 && c <= 8132)))))))))))
        : (c <= 8140 || (c < 12337
          ? (c < 8544
            ? (c < 8458
              ? (c < 8305
                ? (c < 8160
                  ? (c < 8150
                    ? (c >= 8144 && c <= 8147)
                    : c <= 8155)
                  : (c <= 8172 || (c < 8182
                    ? (c >= 8178 && c <= 8180)
                    : c <= 8188)))
                : (c <= 8305 || (c < 8450
                  ? (c < 8336
                    ? c == 8319
                    : c <= 8348)
                  : (c <= 8450 || c == 8455))))
              : (c <= 8467 || (c < 8488
                ? (c < 8484
                  ? (c < 8472
                    ? c == 8469
                    : c <= 8477)
                  : (c <= 8484 || c == 8486))
                : (c <= 8488 || (c < 8517
                  ? (c < 8508
                    ? (c >= 8490 && c <= 8505)
                    : c <= 8511)
                  : (c <= 8521 || c == 8526))))))
            : (c <= 8584 || (c < 11680
              ? (c < 11559
                ? (c < 11506
                  ? (c < 11499
                    ? (c >= 11264 && c <= 11492)
                    : c <= 11502)
                  : (c <= 11507 || (c >= 11520 && c <= 11557)))
                : (c <= 11559 || (c < 11631
                  ? (c < 11568
                    ? c == 11565
                    : c <= 11623)
                  : (c <= 11631 || (c >= 11648 && c <= 11670)))))
              : (c <= 11686 || (c < 11720
                ? (c < 11704
                  ? (c < 11696
                    ? (c >= 11688 && c <= 11694)
                    : c <= 11702)
                  : (c <= 11710 || (c >= 11712 && c <= 11718)))
                : (c <= 11726 || (c < 12293
                  ? (c < 11736
                    ? (c >= 11728 && c <= 11734)
                    : c <= 11742)
                  : (c <= 12295 || (c >= 12321 && c <= 12329)))))))))
          : (c <= 12341 || (c < 42891
            ? (c < 19968
              ? (c < 12549
                ? (c < 12445
                  ? (c < 12353
                    ? (c >= 12344 && c <= 12348)
                    : c <= 12438)
                  : (c <= 12447 || (c < 12540
                    ? (c >= 12449 && c <= 12538)
                    : c <= 12543)))
                : (c <= 12591 || (c < 12784
                  ? (c < 12704
                    ? (c >= 12593 && c <= 12686)
                    : c <= 12735)
                  : (c <= 12799 || (c >= 13312 && c <= 19903)))))
              : (c <= 42124 || (c < 42560
                ? (c < 42512
                  ? (c < 42240
                    ? (c >= 42192 && c <= 42237)
                    : c <= 42508)
                  : (c <= 42527 || (c >= 42538 && c <= 42539)))
                : (c <= 42606 || (c < 42775
                  ? (c < 42656
                    ? (c >= 42623 && c <= 42653)
                    : c <= 42735)
                  : (c <= 42783 || (c >= 42786 && c <= 42888)))))))
            : (c <= 42954 || (c < 43250
              ? (c < 43011
                ? (c < 42965
                  ? (c < 42963
                    ? (c >= 42960 && c <= 42961)
                    : c <= 42963)
                  : (c <= 42969 || (c >= 42994 && c <= 43009)))
                : (c <= 43013 || (c < 43072
                  ? (c < 43020
                    ? (c >= 43015 && c <= 43018)
                    : c <= 43042)
                  : (c <= 43123 || (c >= 43138 && c <= 43187)))))
              : (c <= 43255 || (c < 43360
                ? (c < 43274
                  ? (c < 43261
                    ? c == 43259
                    : c <= 43262)
                  : (c <= 43301 || (c >= 43312 && c <= 43334)))
                : (c <= 43388 || (c < 43488
                  ? (c < 43471
                    ? (c >= 43396 && c <= 43442)
                    : c <= 43471)
                  : (c <= 43492 || (c >= 43494 && c <= 43503)))))))))))))))
    : (c <= 43518 || (c < 70727
      ? (c < 66956
        ? (c < 64914
          ? (c < 43868
            ? (c < 43714
              ? (c < 43646
                ? (c < 43588
                  ? (c < 43584
                    ? (c >= 43520 && c <= 43560)
                    : c <= 43586)
                  : (c <= 43595 || (c < 43642
                    ? (c >= 43616 && c <= 43638)
                    : c <= 43642)))
                : (c <= 43695 || (c < 43705
                  ? (c < 43701
                    ? c == 43697
                    : c <= 43702)
                  : (c <= 43709 || c == 43712))))
              : (c <= 43714 || (c < 43785
                ? (c < 43762
                  ? (c < 43744
                    ? (c >= 43739 && c <= 43741)
                    : c <= 43754)
                  : (c <= 43764 || (c >= 43777 && c <= 43782)))
                : (c <= 43790 || (c < 43816
                  ? (c < 43808
                    ? (c >= 43793 && c <= 43798)
                    : c <= 43814)
                  : (c <= 43822 || (c >= 43824 && c <= 43866)))))))
            : (c <= 43881 || (c < 64287
              ? (c < 63744
                ? (c < 55216
                  ? (c < 44032
                    ? (c >= 43888 && c <= 44002)
                    : c <= 55203)
                  : (c <= 55238 || (c >= 55243 && c <= 55291)))
                : (c <= 64109 || (c < 64275
                  ? (c < 64256
                    ? (c >= 64112 && c <= 64217)
                    : c <= 64262)
                  : (c <= 64279 || c == 64285))))
              : (c <= 64296 || (c < 64323
                ? (c < 64318
                  ? (c < 64312
                    ? (c >= 64298 && c <= 64310)
                    : c <= 64316)
                  : (c <= 64318 || (c >= 64320 && c <= 64321)))
                : (c <= 64324 || (c < 64612
                  ? (c < 64467
                    ? (c >= 64326 && c <= 64433)
                    : c <= 64605)
                  : (c <= 64829 || (c >= 64848 && c <= 64911)))))))))
          : (c <= 64967 || (c < 65599
            ? (c < 65382
              ? (c < 65147
                ? (c < 65139
                  ? (c < 65137
                    ? (c >= 65008 && c <= 65017)
                    : c <= 65137)
                  : (c <= 65139 || (c < 65145
                    ? c == 65143
                    : c <= 65145)))
                : (c <= 65147 || (c < 65313
                  ? (c < 65151
                    ? c == 65149
                    : c <= 65276)
                  : (c <= 65338 || (c >= 65345 && c <= 65370)))))
              : (c <= 65437 || (c < 65498
                ? (c < 65482
                  ? (c < 65474
                    ? (c >= 65440 && c <= 65470)
                    : c <= 65479)
                  : (c <= 65487 || (c >= 65490 && c <= 65495)))
                : (c <= 65500 || (c < 65576
                  ? (c < 65549
                    ? (c >= 65536 && c <= 65547)
                    : c <= 65574)
                  : (c <= 65594 || (c >= 65596 && c <= 65597)))))))
            : (c <= 65613 || (c < 66464
              ? (c < 66208
                ? (c < 65856
                  ? (c < 65664
                    ? (c >= 65616 && c <= 65629)
                    : c <= 65786)
                  : (c <= 65908 || (c >= 66176 && c <= 66204)))
                : (c <= 66256 || (c < 66384
                  ? (c < 66349
                    ? (c >= 66304 && c <= 66335)
                    : c <= 66378)
                  : (c <= 66421 || (c >= 66432 && c <= 66461)))))
              : (c <= 66499 || (c < 66776
                ? (c < 66560
                  ? (c < 66513
                    ? (c >= 66504 && c <= 66511)
                    : c <= 66517)
                  : (c <= 66717 || (c >= 66736 && c <= 66771)))
                : (c <= 66811 || (c < 66928
                  ? (c < 66864
                    ? (c >= 66816 && c <= 66855)
                    : c <= 66915)
                  : (c <= 66938 || (c >= 66940 && c <= 66954)))))))))))
        : (c <= 66962 || (c < 68864
          ? (c < 67828
            ? (c < 67506
              ? (c < 67072
                ? (c < 66979
                  ? (c < 66967
                    ? (c >= 66964 && c <= 66965)
                    : c <= 66977)
                  : (c <= 66993 || (c < 67003
                    ? (c >= 66995 && c <= 67001)
                    : c <= 67004)))
                : (c <= 67382 || (c < 67456
                  ? (c < 67424
                    ? (c >= 67392 && c <= 67413)
                    : c <= 67431)
                  : (c <= 67461 || (c >= 67463 && c <= 67504)))))
              : (c <= 67514 || (c < 67644
                ? (c < 67594
                  ? (c < 67592
                    ? (c >= 67584 && c <= 67589)
                    : c <= 67592)
                  : (c <= 67637 || (c >= 67639 && c <= 67640)))
                : (c <= 67644 || (c < 67712
                  ? (c < 67680
                    ? (c >= 67647 && c <= 67669)
                    : c <= 67702)
                  : (c <= 67742 || (c >= 67808 && c <= 67826)))))))
            : (c <= 67829 || (c < 68224
              ? (c < 68096
                ? (c < 67968
                  ? (c < 67872
                    ? (c >= 67840 && c <= 67861)
                    : c <= 67897)
                  : (c <= 68023 || (c >= 68030 && c <= 68031)))
                : (c <= 68096 || (c < 68121
                  ? (c < 68117
                    ? (c >= 68112 && c <= 68115)
                    : c <= 68119)
                  : (c <= 68149 || (c >= 68192 && c <= 68220)))))
              : (c <= 68252 || (c < 68448
                ? (c < 68352
                  ? (c < 68297
                    ? (c >= 68288 && c <= 68295)
                    : c <= 68324)
                  : (c <= 68405 || (c >= 68416 && c <= 68437)))
                : (c <= 68466 || (c < 68736
                  ? (c < 68608
                    ? (c >= 68480 && c <= 68497)
                    : c <= 68680)
                  : (c <= 68786 || (c >= 68800 && c <= 68850)))))))))
          : (c <= 68899 || (c < 70106
            ? (c < 69749
              ? (c < 69488
                ? (c < 69376
                  ? (c < 69296
                    ? (c >= 69248 && c <= 69289)
                    : c <= 69297)
                  : (c <= 69404 || (c < 69424
                    ? c == 69415
                    : c <= 69445)))
                : (c <= 69505 || (c < 69635
                  ? (c < 69600
                    ? (c >= 69552 && c <= 69572)
                    : c <= 69622)
                  : (c <= 69687 || (c >= 69745 && c <= 69746)))))
              : (c <= 69749 || (c < 69959
                ? (c < 69891
                  ? (c < 69840
                    ? (c >= 69763 && c <= 69807)
                    : c <= 69864)
                  : (c <= 69926 || c == 69956))
                : (c <= 69959 || (c < 70019
                  ? (c < 70006
                    ? (c >= 69968 && c <= 70002)
                    : c <= 70006)
                  : (c <= 70066 || (c >= 70081 && c <= 70084)))))))
            : (c <= 70106 || (c < 70405
              ? (c < 70280
                ? (c < 70163
                  ? (c < 70144
                    ? c == 70108
                    : c <= 70161)
                  : (c <= 70187 || (c >= 70272 && c <= 70278)))
                : (c <= 70280 || (c < 70303
                  ? (c < 70287
                    ? (c >= 70282 && c <= 70285)
                    : c <= 70301)
                  : (c <= 70312 || (c >= 70320 && c <= 70366)))))
              : (c <= 70412 || (c < 70453
                ? (c < 70442
                  ? (c < 70419
                    ? (c >= 70415 && c <= 70416)
                    : c <= 70440)
                  : (c <= 70448 || (c >= 70450 && c <= 70451)))
                : (c <= 70457 || (c < 70493
                  ? (c < 70480
                    ? c == 70461
                    : c <= 70480)
                  : (c <= 70497 || (c >= 70656 && c <= 70708)))))))))))))
      : (c <= 70730 || (c < 119894
        ? (c < 73056
          ? (c < 72001
            ? (c < 71424
              ? (c < 71128
                ? (c < 70852
                  ? (c < 70784
                    ? (c >= 70751 && c <= 70753)
                    : c <= 70831)
                  : (c <= 70853 || (c < 71040
                    ? c == 70855
                    : c <= 71086)))
                : (c <= 71131 || (c < 71296
                  ? (c < 71236
                    ? (c >= 71168 && c <= 71215)
                    : c <= 71236)
                  : (c <= 71338 || c == 71352))))
              : (c <= 71450 || (c < 71945
                ? (c < 71840
                  ? (c < 71680
                    ? (c >= 71488 && c <= 71494)
                    : c <= 71723)
                  : (c <= 71903 || (c >= 71935 && c <= 71942)))
                : (c <= 71945 || (c < 71960
                  ? (c < 71957
                    ? (c >= 71948 && c <= 71955)
                    : c <= 71958)
                  : (c <= 71983 || c == 71999))))))
            : (c <= 72001 || (c < 72349
              ? (c < 72192
                ? (c < 72161
                  ? (c < 72106
                    ? (c >= 72096 && c <= 72103)
                    : c <= 72144)
                  : (c <= 72161 || c == 72163))
                : (c <= 72192 || (c < 72272
                  ? (c < 72250
                    ? (c >= 72203 && c <= 72242)
                    : c <= 72250)
                  : (c <= 72272 || (c >= 72284 && c <= 72329)))))
              : (c <= 72349 || (c < 72818
                ? (c < 72714
                  ? (c < 72704
                    ? (c >= 72368 && c <= 72440)
                    : c <= 72712)
                  : (c <= 72750 || c == 72768))
                : (c <= 72847 || (c < 72971
                  ? (c < 72968
                    ? (c >= 72960 && c <= 72966)
                    : c <= 72969)
                  : (c <= 73008 || c == 73030))))))))
          : (c <= 73061 || (c < 93952
            ? (c < 82944
              ? (c < 73728
                ? (c < 73112
                  ? (c < 73066
                    ? (c >= 73063 && c <= 73064)
                    : c <= 73097)
                  : (c <= 73112 || (c < 73648
                    ? (c >= 73440 && c <= 73458)
                    : c <= 73648)))
                : (c <= 74649 || (c < 77712
                  ? (c < 74880
                    ? (c >= 74752 && c <= 74862)
                    : c <= 75075)
                  : (c <= 77808 || (c >= 77824 && c <= 78894)))))
              : (c <= 83526 || (c < 92928
                ? (c < 92784
                  ? (c < 92736
                    ? (c >= 92160 && c <= 92728)
                    : c <= 92766)
                  : (c <= 92862 || (c >= 92880 && c <= 92909)))
                : (c <= 92975 || (c < 93053
                  ? (c < 93027
                    ? (c >= 92992 && c <= 92995)
                    : c <= 93047)
                  : (c <= 93071 || (c >= 93760 && c <= 93823)))))))
            : (c <= 94026 || (c < 110589
              ? (c < 94208
                ? (c < 94176
                  ? (c < 94099
                    ? c == 94032
                    : c <= 94111)
                  : (c <= 94177 || c == 94179))
                : (c <= 100343 || (c < 110576
                  ? (c < 101632
                    ? (c >= 100352 && c <= 101589)
                    : c <= 101640)
                  : (c <= 110579 || (c >= 110581 && c <= 110587)))))
              : (c <= 110590 || (c < 113664
                ? (c < 110948
                  ? (c < 110928
                    ? (c >= 110592 && c <= 110882)
                    : c <= 110930)
                  : (c <= 110951 || (c >= 110960 && c <= 111355)))
                : (c <= 113770 || (c < 113808
                  ? (c < 113792
                    ? (c >= 113776 && c <= 113788)
                    : c <= 113800)
                  : (c <= 113817 || (c >= 119808 && c <= 119892)))))))))))
        : (c <= 119964 || (c < 125259
          ? (c < 120572
            ? (c < 120086
              ? (c < 119995
                ? (c < 119973
                  ? (c < 119970
                    ? (c >= 119966 && c <= 119967)
                    : c <= 119970)
                  : (c <= 119974 || (c < 119982
                    ? (c >= 119977 && c <= 119980)
                    : c <= 119993)))
                : (c <= 119995 || (c < 120071
                  ? (c < 120005
                    ? (c >= 119997 && c <= 120003)
                    : c <= 120069)
                  : (c <= 120074 || (c >= 120077 && c <= 120084)))))
              : (c <= 120092 || (c < 120138
                ? (c < 120128
                  ? (c < 120123
                    ? (c >= 120094 && c <= 120121)
                    : c <= 120126)
                  : (c <= 120132 || c == 120134))
                : (c <= 120144 || (c < 120514
                  ? (c < 120488
                    ? (c >= 120146 && c <= 120485)
                    : c <= 120512)
                  : (c <= 120538 || (c >= 120540 && c <= 120570)))))))
            : (c <= 120596 || (c < 123191
              ? (c < 120714
                ? (c < 120656
                  ? (c < 120630
                    ? (c >= 120598 && c <= 120628)
                    : c <= 120654)
                  : (c <= 120686 || (c >= 120688 && c <= 120712)))
                : (c <= 120744 || (c < 122624
                  ? (c < 120772
                    ? (c >= 120746 && c <= 120770)
                    : c <= 120779)
                  : (c <= 122654 || (c >= 123136 && c <= 123180)))))
              : (c <= 123197 || (c < 124904
                ? (c < 123584
                  ? (c < 123536
                    ? c == 123214
                    : c <= 123565)
                  : (c <= 123627 || (c >= 124896 && c <= 124902)))
                : (c <= 124907 || (c < 124928
                  ? (c < 124912
                    ? (c >= 124909 && c <= 124910)
                    : c <= 124926)
                  : (c <= 125124 || (c >= 125184 && c <= 125251)))))))))
          : (c <= 125259 || (c < 126559
            ? (c < 126535
              ? (c < 126505
                ? (c < 126497
                  ? (c < 126469
                    ? (c >= 126464 && c <= 126467)
                    : c <= 126495)
                  : (c <= 126498 || (c < 126503
                    ? c == 126500
                    : c <= 126503)))
                : (c <= 126514 || (c < 126523
                  ? (c < 126521
                    ? (c >= 126516 && c <= 126519)
                    : c <= 126521)
                  : (c <= 126523 || c == 126530))))
              : (c <= 126535 || (c < 126548
                ? (c < 126541
                  ? (c < 126539
                    ? c == 126537
                    : c <= 126539)
                  : (c <= 126543 || (c >= 126545 && c <= 126546)))
                : (c <= 126548 || (c < 126555
                  ? (c < 126553
                    ? c == 126551
                    : c <= 126553)
                  : (c <= 126555 || c == 126557))))))
            : (c <= 126559 || (c < 126625
              ? (c < 126580
                ? (c < 126567
                  ? (c < 126564
                    ? (c >= 126561 && c <= 126562)
                    : c <= 126564)
                  : (c <= 126570 || (c >= 126572 && c <= 126578)))
                : (c <= 126583 || (c < 126592
                  ? (c < 126590
                    ? (c >= 126585 && c <= 126588)
                    : c <= 126590)
                  : (c <= 126601 || (c >= 126603 && c <= 126619)))))
              : (c <= 126627 || (c < 177984
                ? (c < 131072
                  ? (c < 126635
                    ? (c >= 126629 && c <= 126633)
                    : c <= 126651)
                  : (c <= 173791 || (c >= 173824 && c <= 177976)))
                : (c <= 178205 || (c < 194560
                  ? (c < 183984
                    ? (c >= 178208 && c <= 183969)
                    : c <= 191456)
                  : (c <= 195101 || (c >= 196608 && c <= 201546)))))))))))))))));
}

static inline bool sym_ident_character_set_2(int32_t c) {
  return (c < 43616
    ? (c < 3782
      ? (c < 2748
        ? (c < 2045
          ? (c < 1015
            ? (c < 710
              ? (c < 181
                ? (c < '_'
                  ? (c < 'A'
                    ? (c >= '0' && c <= '9')
                    : c <= 'Z')
                  : (c <= '_' || (c < 170
                    ? (c >= 'a' && c <= 'z')
                    : c <= 170)))
                : (c <= 181 || (c < 192
                  ? (c < 186
                    ? c == 183
                    : c <= 186)
                  : (c <= 214 || (c < 248
                    ? (c >= 216 && c <= 246)
                    : c <= 705)))))
              : (c <= 721 || (c < 891
                ? (c < 750
                  ? (c < 748
                    ? (c >= 736 && c <= 740)
                    : c <= 748)
                  : (c <= 750 || (c < 886
                    ? (c >= 768 && c <= 884)
                    : c <= 887)))
                : (c <= 893 || (c < 908
                  ? (c < 902
                    ? c == 895
                    : c <= 906)
                  : (c <= 908 || (c < 931
                    ? (c >= 910 && c <= 929)
                    : c <= 1013)))))))
            : (c <= 1153 || (c < 1519
              ? (c < 1425
                ? (c < 1329
                  ? (c < 1162
                    ? (c >= 1155 && c <= 1159)
                    : c <= 1327)
                  : (c <= 1366 || (c < 1376
                    ? c == 1369
                    : c <= 1416)))
                : (c <= 1469 || (c < 1476
                  ? (c < 1473
                    ? c == 1471
                    : c <= 1474)
                  : (c <= 1477 || (c < 1488
                    ? c == 1479
                    : c <= 1514)))))
              : (c <= 1522 || (c < 1770
                ? (c < 1646
                  ? (c < 1568
                    ? (c >= 1552 && c <= 1562)
                    : c <= 1641)
                  : (c <= 1747 || (c < 1759
                    ? (c >= 1749 && c <= 1756)
                    : c <= 1768)))
                : (c <= 1788 || (c < 1869
                  ? (c < 1808
                    ? c == 1791
                    : c <= 1866)
                  : (c <= 1969 || (c < 2042
                    ? (c >= 1984 && c <= 2037)
                    : c <= 2042)))))))))
          : (c <= 2045 || (c < 2558
            ? (c < 2451
              ? (c < 2200
                ? (c < 2144
                  ? (c < 2112
                    ? (c >= 2048 && c <= 2093)
                    : c <= 2139)
                  : (c <= 2154 || (c < 2185
                    ? (c >= 2160 && c <= 2183)
                    : c <= 2190)))
                : (c <= 2273 || (c < 2417
                  ? (c < 2406
                    ? (c >= 2275 && c <= 2403)
                    : c <= 2415)
                  : (c <= 2435 || (c < 2447
                    ? (c >= 2437 && c <= 2444)
                    : c <= 2448)))))
              : (c <= 2472 || (c < 2507
                ? (c < 2486
                  ? (c < 2482
                    ? (c >= 2474 && c <= 2480)
                    : c <= 2482)
                  : (c <= 2489 || (c < 2503
                    ? (c >= 2492 && c <= 2500)
                    : c <= 2504)))
                : (c <= 2510 || (c < 2527
                  ? (c < 2524
                    ? c == 2519
                    : c <= 2525)
                  : (c <= 2531 || (c < 2556
                    ? (c >= 2534 && c <= 2545)
                    : c <= 2556)))))))
            : (c <= 2558 || (c < 2635
              ? (c < 2610
                ? (c < 2575
                  ? (c < 2565
                    ? (c >= 2561 && c <= 2563)
                    : c <= 2570)
                  : (c <= 2576 || (c < 2602
                    ? (c >= 2579 && c <= 2600)
                    : c <= 2608)))
                : (c <= 2611 || (c < 2620
                  ? (c < 2616
                    ? (c >= 2613 && c <= 2614)
                    : c <= 2617)
                  : (c <= 2620 || (c < 2631
                    ? (c >= 2622 && c <= 2626)
                    : c <= 2632)))))
              : (c <= 2637 || (c < 2693
                ? (c < 2654
                  ? (c < 2649
                    ? c == 2641
                    : c <= 2652)
                  : (c <= 2654 || (c < 2689
                    ? (c >= 2662 && c <= 2677)
                    : c <= 2691)))
                : (c <= 2701 || (c < 2730
                  ? (c < 2707
                    ? (c >= 2703 && c <= 2705)
                    : c <= 2728)
                  : (c <= 2736 || (c < 2741
                    ? (c >= 2738 && c <= 2739)
                    : c <= 2745)))))))))))
        : (c <= 2757 || (c < 3168
          ? (c < 2958
            ? (c < 2866
              ? (c < 2809
                ? (c < 2768
                  ? (c < 2763
                    ? (c >= 2759 && c <= 2761)
                    : c <= 2765)
                  : (c <= 2768 || (c < 2790
                    ? (c >= 2784 && c <= 2787)
                    : c <= 2799)))
                : (c <= 2815 || (c < 2831
                  ? (c < 2821
                    ? (c >= 2817 && c <= 2819)
                    : c <= 2828)
                  : (c <= 2832 || (c < 2858
                    ? (c >= 2835 && c <= 2856)
                    : c <= 2864)))))
              : (c <= 2867 || (c < 2908
                ? (c < 2887
                  ? (c < 2876
                    ? (c >= 2869 && c <= 2873)
                    : c <= 2884)
                  : (c <= 2888 || (c < 2901
                    ? (c >= 2891 && c <= 2893)
                    : c <= 2903)))
                : (c <= 2909 || (c < 2929
                  ? (c < 2918
                    ? (c >= 2911 && c <= 2915)
                    : c <= 2927)
                  : (c <= 2929 || (c < 2949
                    ? (c >= 2946 && c <= 2947)
                    : c <= 2954)))))))
            : (c <= 2960 || (c < 3031
              ? (c < 2984
                ? (c < 2972
                  ? (c < 2969
                    ? (c >= 2962 && c <= 2965)
                    : c <= 2970)
                  : (c <= 2972 || (c < 2979
                    ? (c >= 2974 && c <= 2975)
                    : c <= 2980)))
                : (c <= 2986 || (c < 3014
                  ? (c < 3006
                    ? (c >= 2990 && c <= 3001)
                    : c <= 3010)
                  : (c <= 3016 || (c < 3024
                    ? (c >= 3018 && c <= 3021)
                    : c <= 3024)))))
              : (c <= 3031 || (c < 3132
                ? (c < 3086
                  ? (c < 3072
                    ? (c >= 3046 && c <= 3055)
                    : c <= 3084)
                  : (c <= 3088 || (c < 3114
                    ? (c >= 3090 && c <= 3112)
                    : c <= 3129)))
                : (c <= 3140 || (c < 3157
                  ? (c < 3146
                    ? (c >= 3142 && c <= 3144)
                    : c <= 3149)
                  : (c <= 3158 || (c < 3165
                    ? (c >= 3160 && c <= 3162)
                    : c <= 3165)))))))))
          : (c <= 3171 || (c < 3450
            ? (c < 3293
              ? (c < 3242
                ? (c < 3205
                  ? (c < 3200
                    ? (c >= 3174 && c <= 3183)
                    : c <= 3203)
                  : (c <= 3212 || (c < 3218
                    ? (c >= 3214 && c <= 3216)
                    : c <= 3240)))
                : (c <= 3251 || (c < 3270
                  ? (c < 3260
                    ? (c >= 3253 && c <= 3257)
                    : c <= 3268)
                  : (c <= 3272 || (c < 3285
                    ? (c >= 3274 && c <= 3277)
                    : c <= 3286)))))
              : (c <= 3294 || (c < 3346
                ? (c < 3313
                  ? (c < 3302
                    ? (c >= 3296 && c <= 3299)
                    : c <= 3311)
                  : (c <= 3314 || (c < 3342
                    ? (c >= 3328 && c <= 3340)
                    : c <= 3344)))
                : (c <= 3396 || (c < 3412
                  ? (c < 3402
                    ? (c >= 3398 && c <= 3400)
                    : c <= 3406)
                  : (c <= 3415 || (c < 3430
                    ? (c >= 3423 && c <= 3427)
                    : c <= 3439)))))))
            : (c <= 3455 || (c < 3570
              ? (c < 3520
                ? (c < 3482
                  ? (c < 3461
                    ? (c >= 3457 && c <= 3459)
                    : c <= 3478)
                  : (c <= 3505 || (c < 3517
                    ? (c >= 3507 && c <= 3515)
                    : c <= 3517)))
                : (c <= 3526 || (c < 3542
                  ? (c < 3535
                    ? c == 3530
                    : c <= 3540)
                  : (c <= 3542 || (c < 3558
                    ? (c >= 3544 && c <= 3551)
                    : c <= 3567)))))
              : (c <= 3571 || (c < 3718
                ? (c < 3664
                  ? (c < 3648
                    ? (c >= 3585 && c <= 3642)
                    : c <= 3662)
                  : (c <= 3673 || (c < 3716
                    ? (c >= 3713 && c <= 3714)
                    : c <= 3716)))
                : (c <= 3722 || (c < 3751
                  ? (c < 3749
                    ? (c >= 3724 && c <= 3747)
                    : c <= 3749)
                  : (c <= 3773 || (c >= 3776 && c <= 3780)))))))))))))
      : (c <= 3782 || (c < 8025
        ? (c < 5888
          ? (c < 4688
            ? (c < 3953
              ? (c < 3872
                ? (c < 3804
                  ? (c < 3792
                    ? (c >= 3784 && c <= 3789)
                    : c <= 3801)
                  : (c <= 3807 || (c < 3864
                    ? c == 3840
                    : c <= 3865)))
                : (c <= 3881 || (c < 3897
                  ? (c < 3895
                    ? c == 3893
                    : c <= 3895)
                  : (c <= 3897 || (c < 3913
                    ? (c >= 3902 && c <= 3911)
                    : c <= 3948)))))
              : (c <= 3972 || (c < 4256
                ? (c < 4038
                  ? (c < 3993
                    ? (c >= 3974 && c <= 3991)
                    : c <= 4028)
                  : (c <= 4038 || (c < 4176
                    ? (c >= 4096 && c <= 4169)
                    : c <= 4253)))
                : (c <= 4293 || (c < 4304
                  ? (c < 4301
                    ? c == 4295
                    : c <= 4301)
                  : (c <= 4346 || (c < 4682
                    ? (c >= 4348 && c <= 4680)
                    : c <= 4685)))))))
            : (c <= 4694 || (c < 4882
              ? (c < 4786
                ? (c < 4704
                  ? (c < 4698
                    ? c == 4696
                    : c <= 4701)
                  : (c <= 4744 || (c < 4752
                    ? (c >= 4746 && c <= 4749)
                    : c <= 4784)))
                : (c <= 4789 || (c < 4802
                  ? (c < 4800
                    ? (c >= 4792 && c <= 4798)
                    : c <= 4800)
                  : (c <= 4805 || (c < 4824
                    ? (c >= 4808 && c <= 4822)
                    : c <= 4880)))))
              : (c <= 4885 || (c < 5112
                ? (c < 4969
                  ? (c < 4957
                    ? (c >= 4888 && c <= 4954)
                    : c <= 4959)
                  : (c <= 4977 || (c < 5024
                    ? (c >= 4992 && c <= 5007)
                    : c <= 5109)))
                : (c <= 5117 || (c < 5761
                  ? (c < 5743
                    ? (c >= 5121 && c <= 5740)
                    : c <= 5759)
                  : (c <= 5786 || (c < 5870
                    ? (c >= 5792 && c <= 5866)
                    : c <= 5880)))))))))
          : (c <= 5909 || (c < 6688
            ? (c < 6176
              ? (c < 6016
                ? (c < 5984
                  ? (c < 5952
                    ? (c >= 5919 && c <= 5940)
                    : c <= 5971)
                  : (c <= 5996 || (c < 6002
                    ? (c >= 5998 && c <= 6000)
                    : c <= 6003)))
                : (c <= 6099 || (c < 6112
                  ? (c < 6108
                    ? c == 6103
                    : c <= 6109)
                  : (c <= 6121 || (c < 6159
                    ? (c >= 6155 && c <= 6157)
                    : c <= 6169)))))
              : (c <= 6264 || (c < 6470
                ? (c < 6400
                  ? (c < 6320
                    ? (c >= 6272 && c <= 6314)
                    : c <= 6389)
                  : (c <= 6430 || (c < 6448
                    ? (c >= 6432 && c <= 6443)
                    : c <= 6459)))
                : (c <= 6509 || (c < 6576
                  ? (c < 6528
                    ? (c >= 6512 && c <= 6516)
                    : c <= 6571)
                  : (c <= 6601 || (c < 6656
                    ? (c >= 6608 && c <= 6618)
                    : c <= 6683)))))))
            : (c <= 6750 || (c < 7232
              ? (c < 6847
                ? (c < 6800
                  ? (c < 6783
                    ? (c >= 6752 && c <= 6780)
                    : c <= 6793)
                  : (c <= 6809 || (c < 6832
                    ? c == 6823
                    : c <= 6845)))
                : (c <= 6862 || (c < 7019
                  ? (c < 6992
                    ? (c >= 6912 && c <= 6988)
                    : c <= 7001)
                  : (c <= 7027 || (c < 7168
                    ? (c >= 7040 && c <= 7155)
                    : c <= 7223)))))
              : (c <= 7241 || (c < 7380
                ? (c < 7312
                  ? (c < 7296
                    ? (c >= 7245 && c <= 7293)
                    : c <= 7304)
                  : (c <= 7354 || (c < 7376
                    ? (c >= 7357 && c <= 7359)
                    : c <= 7378)))
                : (c <= 7418 || (c < 7968
                  ? (c < 7960
                    ? (c >= 7424 && c <= 7957)
                    : c <= 7965)
                  : (c <= 8005 || (c < 8016
                    ? (c >= 8008 && c <= 8013)
                    : c <= 8023)))))))))))
        : (c <= 8025 || (c < 11720
          ? (c < 8458
            ? (c < 8178
              ? (c < 8126
                ? (c < 8031
                  ? (c < 8029
                    ? c == 8027
                    : c <= 8029)
                  : (c <= 8061 || (c < 8118
                    ? (c >= 8064 && c <= 8116)
                    : c <= 8124)))
                : (c <= 8126 || (c < 8144
                  ? (c < 8134
                    ? (c >= 8130 && c <= 8132)
                    : c <= 8140)
                  : (c <= 8147 || (c < 8160
                    ? (c >= 8150 && c <= 8155)
                    : c <= 8172)))))
              : (c <= 8180 || (c < 8336
                ? (c < 8276
                  ? (c < 8255
                    ? (c >= 8182 && c <= 8188)
                    : c <= 8256)
                  : (c <= 8276 || (c < 8319
                    ? c == 8305
                    : c <= 8319)))
                : (c <= 8348 || (c < 8421
                  ? (c < 8417
                    ? (c >= 8400 && c <= 8412)
                    : c <= 8417)
                  : (c <= 8432 || (c < 8455
                    ? c == 8450
                    : c <= 8455)))))))
            : (c <= 8467 || (c < 11499
              ? (c < 8490
                ? (c < 8484
                  ? (c < 8472
                    ? c == 8469
                    : c <= 8477)
                  : (c <= 8484 || (c < 8488
                    ? c == 8486
                    : c <= 8488)))
                : (c <= 8505 || (c < 8526
                  ? (c < 8517
                    ? (c >= 8508 && c <= 8511)
                    : c <= 8521)
                  : (c <= 8526 || (c < 11264
                    ? (c >= 8544 && c <= 8584)
                    : c <= 11492)))))
              : (c <= 11507 || (c < 11647
                ? (c < 11565
                  ? (c < 11559
                    ? (c >= 11520 && c <= 11557)
                    : c <= 11559)
                  : (c <= 11565 || (c < 11631
                    ? (c >= 11568 && c <= 11623)
                    : c <= 11631)))
                : (c <= 11670 || (c < 11696
                  ? (c < 11688
                    ? (c >= 11680 && c <= 11686)
                    : c <= 11694)
                  : (c <= 11702 || (c < 11712
                    ? (c >= 11704 && c <= 11710)
                    : c <= 11718)))))))))
          : (c <= 11726 || (c < 42623
            ? (c < 12540
              ? (c < 12337
                ? (c < 11744
                  ? (c < 11736
                    ? (c >= 11728 && c <= 11734)
                    : c <= 11742)
                  : (c <= 11775 || (c < 12321
                    ? (c >= 12293 && c <= 12295)
                    : c <= 12335)))
                : (c <= 12341 || (c < 12441
                  ? (c < 12353
                    ? (c >= 12344 && c <= 12348)
                    : c <= 12438)
                  : (c <= 12442 || (c < 12449
                    ? (c >= 12445 && c <= 12447)
                    : c <= 12538)))))
              : (c <= 12543 || (c < 19968
                ? (c < 12704
                  ? (c < 12593
                    ? (c >= 12549 && c <= 12591)
                    : c <= 12686)
                  : (c <= 12735 || (c < 13312
                    ? (c >= 12784 && c <= 12799)
                    : c <= 19903)))
                : (c <= 42124 || (c < 42512
                  ? (c < 42240
                    ? (c >= 42192 && c <= 42237)
                    : c <= 42508)
                  : (c <= 42539 || (c < 42612
                    ? (c >= 42560 && c <= 42607)
                    : c <= 42621)))))))
            : (c <= 42737 || (c < 43232
              ? (c < 42965
                ? (c < 42891
                  ? (c < 42786
                    ? (c >= 42775 && c <= 42783)
                    : c <= 42888)
                  : (c <= 42954 || (c < 42963
                    ? (c >= 42960 && c <= 42961)
                    : c <= 42963)))
                : (c <= 42969 || (c < 43072
                  ? (c < 43052
                    ? (c >= 42994 && c <= 43047)
                    : c <= 43052)
                  : (c <= 43123 || (c < 43216
                    ? (c >= 43136 && c <= 43205)
                    : c <= 43225)))))
              : (c <= 43255 || (c < 43471
                ? (c < 43312
                  ? (c < 43261
                    ? c == 43259
                    : c <= 43309)
                  : (c <= 43347 || (c < 43392
                    ? (c >= 43360 && c <= 43388)
                    : c <= 43456)))
                : (c <= 43481 || (c < 43584
                  ? (c < 43520
                    ? (c >= 43488 && c <= 43518)
                    : c <= 43574)
                  : (c <= 43597 || (c >= 43600 && c <= 43609)))))))))))))))
    : (c <= 43638 || (c < 71453
      ? (c < 67639
        ? (c < 65345
          ? (c < 64312
            ? (c < 43888
              ? (c < 43785
                ? (c < 43744
                  ? (c < 43739
                    ? (c >= 43642 && c <= 43714)
                    : c <= 43741)
                  : (c <= 43759 || (c < 43777
                    ? (c >= 43762 && c <= 43766)
                    : c <= 43782)))
                : (c <= 43790 || (c < 43816
                  ? (c < 43808
                    ? (c >= 43793 && c <= 43798)
                    : c <= 43814)
                  : (c <= 43822 || (c < 43868
                    ? (c >= 43824 && c <= 43866)
                    : c <= 43881)))))
              : (c <= 44010 || (c < 63744
                ? (c < 44032
                  ? (c < 44016
                    ? (c >= 44012 && c <= 44013)
                    : c <= 44025)
                  : (c <= 55203 || (c < 55243
                    ? (c >= 55216 && c <= 55238)
                    : c <= 55291)))
                : (c <= 64109 || (c < 64275
                  ? (c < 64256
                    ? (c >= 64112 && c <= 64217)
                    : c <= 64262)
                  : (c <= 64279 || (c < 64298
                    ? (c >= 64285 && c <= 64296)
                    : c <= 64310)))))))
            : (c <= 64316 || (c < 65075
              ? (c < 64612
                ? (c < 64323
                  ? (c < 64320
                    ? c == 64318
                    : c <= 64321)
                  : (c <= 64324 || (c < 64467
                    ? (c >= 64326 && c <= 64433)
                    : c <= 64605)))
                : (c <= 64829 || (c < 65008
                  ? (c < 64914
                    ? (c >= 64848 && c <= 64911)
                    : c <= 64967)
                  : (c <= 65017 || (c < 65056
                    ? (c >= 65024 && c <= 65039)
                    : c <= 65071)))))
              : (c <= 65076 || (c < 65147
                ? (c < 65139
                  ? (c < 65137
                    ? (c >= 65101 && c <= 65103)
                    : c <= 65137)
                  : (c <= 65139 || (c < 65145
                    ? c == 65143
                    : c <= 65145)))
                : (c <= 65147 || (c < 65296
                  ? (c < 65151
                    ? c == 65149
                    : c <= 65276)
                  : (c <= 65305 || (c < 65343
                    ? (c >= 65313 && c <= 65338)
                    : c <= 65343)))))))))
          : (c <= 65370 || (c < 66513
            ? (c < 65664
              ? (c < 65536
                ? (c < 65482
                  ? (c < 65474
                    ? (c >= 65382 && c <= 65470)
                    : c <= 65479)
                  : (c <= 65487 || (c < 65498
                    ? (c >= 65490 && c <= 65495)
                    : c <= 65500)))
                : (c <= 65547 || (c < 65596
                  ? (c < 65576
                    ? (c >= 65549 && c <= 65574)
                    : c <= 65594)
                  : (c <= 65597 || (c < 65616
                    ? (c >= 65599 && c <= 65613)
                    : c <= 65629)))))
              : (c <= 65786 || (c < 66304
                ? (c < 66176
                  ? (c < 66045
                    ? (c >= 65856 && c <= 65908)
                    : c <= 66045)
                  : (c <= 66204 || (c < 66272
                    ? (c >= 66208 && c <= 66256)
                    : c <= 66272)))
                : (c <= 66335 || (c < 66432
                  ? (c < 66384
                    ? (c >= 66349 && c <= 66378)
                    : c <= 66426)
                  : (c <= 66461 || (c < 66504
                    ? (c >= 66464 && c <= 66499)
                    : c <= 66511)))))))
            : (c <= 66517 || (c < 66979
              ? (c < 66864
                ? (c < 66736
                  ? (c < 66720
                    ? (c >= 66560 && c <= 66717)
                    : c <= 66729)
                  : (c <= 66771 || (c < 66816
                    ? (c >= 66776 && c <= 66811)
                    : c <= 66855)))
                : (c <= 66915 || (c < 66956
                  ? (c < 66940
                    ? (c >= 66928 && c <= 66938)
                    : c <= 66954)
                  : (c <= 66962 || (c < 66967
                    ? (c >= 66964 && c <= 66965)
                    : c <= 66977)))))
              : (c <= 66993 || (c < 67456
                ? (c < 67072
                  ? (c < 67003
                    ? (c >= 66995 && c <= 67001)
                    : c <= 67004)
                  : (c <= 67382 || (c < 67424
                    ? (c >= 67392 && c <= 67413)
                    : c <= 67431)))
                : (c <= 67461 || (c < 67584
                  ? (c < 67506
                    ? (c >= 67463 && c <= 67504)
                    : c <= 67514)
                  : (c <= 67589 || (c < 67594
                    ? c == 67592
                    : c <= 67637)))))))))))
        : (c <= 67640 || (c < 69956
          ? (c < 68448
            ? (c < 68101
              ? (c < 67828
                ? (c < 67680
                  ? (c < 67647
                    ? c == 67644
                    : c <= 67669)
                  : (c <= 67702 || (c < 67808
                    ? (c >= 67712 && c <= 67742)
                    : c <= 67826)))
                : (c <= 67829 || (c < 67968
                  ? (c < 67872
                    ? (c >= 67840 && c <= 67861)
                    : c <= 67897)
                  : (c <= 68023 || (c < 68096
                    ? (c >= 68030 && c <= 68031)
                    : c <= 68099)))))
              : (c <= 68102 || (c < 68192
                ? (c < 68121
                  ? (c < 68117
                    ? (c >= 68108 && c <= 68115)
                    : c <= 68119)
                  : (c <= 68149 || (c < 68159
                    ? (c >= 68152 && c <= 68154)
                    : c <= 68159)))
                : (c <= 68220 || (c < 68297
                  ? (c < 68288
                    ? (c >= 68224 && c <= 68252)
                    : c <= 68295)
                  : (c <= 68326 || (c < 68416
                    ? (c >= 68352 && c <= 68405)
                    : c <= 68437)))))))
            : (c <= 68466 || (c < 69424
              ? (c < 68912
                ? (c < 68736
                  ? (c < 68608
                    ? (c >= 68480 && c <= 68497)
                    : c <= 68680)
                  : (c <= 68786 || (c < 68864
                    ? (c >= 68800 && c <= 68850)
                    : c <= 68903)))
                : (c <= 68921 || (c < 69296
                  ? (c < 69291
                    ? (c >= 69248 && c <= 69289)
                    : c <= 69292)
                  : (c <= 69297 || (c < 69415
                    ? (c >= 69376 && c <= 69404)
                    : c <= 69415)))))
              : (c <= 69456 || (c < 69759
                ? (c < 69600
                  ? (c < 69552
                    ? (c >= 69488 && c <= 69509)
                    : c <= 69572)
                  : (c <= 69622 || (c < 69734
                    ? (c >= 69632 && c <= 69702)
                    : c <= 69749)))
                : (c <= 69818 || (c < 69872
                  ? (c < 69840
                    ? c == 69826
                    : c <= 69864)
                  : (c <= 69881 || (c < 69942
                    ? (c >= 69888 && c <= 69940)
                    : c <= 69951)))))))))
          : (c <= 69959 || (c < 70459
            ? (c < 70282
              ? (c < 70108
                ? (c < 70016
                  ? (c < 70006
                    ? (c >= 69968 && c <= 70003)
                    : c <= 70006)
                  : (c <= 70084 || (c < 70094
                    ? (c >= 70089 && c <= 70092)
                    : c <= 70106)))
                : (c <= 70108 || (c < 70206
                  ? (c < 70163
                    ? (c >= 70144 && c <= 70161)
                    : c <= 70199)
                  : (c <= 70206 || (c < 70280
                    ? (c >= 70272 && c <= 70278)
                    : c <= 70280)))))
              : (c <= 70285 || (c < 70405
                ? (c < 70320
                  ? (c < 70303
                    ? (c >= 70287 && c <= 70301)
                    : c <= 70312)
                  : (c <= 70378 || (c < 70400
                    ? (c >= 70384 && c <= 70393)
                    : c <= 70403)))
                : (c <= 70412 || (c < 70442
                  ? (c < 70419
                    ? (c >= 70415 && c <= 70416)
                    : c <= 70440)
                  : (c <= 70448 || (c < 70453
                    ? (c >= 70450 && c <= 70451)
                    : c <= 70457)))))))
            : (c <= 70468 || (c < 70855
              ? (c < 70502
                ? (c < 70480
                  ? (c < 70475
                    ? (c >= 70471 && c <= 70472)
                    : c <= 70477)
                  : (c <= 70480 || (c < 70493
                    ? c == 70487
                    : c <= 70499)))
                : (c <= 70508 || (c < 70736
                  ? (c < 70656
                    ? (c >= 70512 && c <= 70516)
                    : c <= 70730)
                  : (c <= 70745 || (c < 70784
                    ? (c >= 70750 && c <= 70753)
                    : c <= 70853)))))
              : (c <= 70855 || (c < 71236
                ? (c < 71096
                  ? (c < 71040
                    ? (c >= 70864 && c <= 70873)
                    : c <= 71093)
                  : (c <= 71104 || (c < 71168
                    ? (c >= 71128 && c <= 71133)
                    : c <= 71232)))
                : (c <= 71236 || (c < 71360
                  ? (c < 71296
                    ? (c >= 71248 && c <= 71257)
                    : c <= 71352)
                  : (c <= 71369 || (c >= 71424 && c <= 71450)))))))))))))
      : (c <= 71467 || (c < 119973
        ? (c < 77824
          ? (c < 72760
            ? (c < 72016
              ? (c < 71945
                ? (c < 71680
                  ? (c < 71488
                    ? (c >= 71472 && c <= 71481)
                    : c <= 71494)
                  : (c <= 71738 || (c < 71935
                    ? (c >= 71840 && c <= 71913)
                    : c <= 71942)))
                : (c <= 71945 || (c < 71960
                  ? (c < 71957
                    ? (c >= 71948 && c <= 71955)
                    : c <= 71958)
                  : (c <= 71989 || (c < 71995
                    ? (c >= 71991 && c <= 71992)
                    : c <= 72003)))))
              : (c <= 72025 || (c < 72263
                ? (c < 72154
                  ? (c < 72106
                    ? (c >= 72096 && c <= 72103)
                    : c <= 72151)
                  : (c <= 72161 || (c < 72192
                    ? (c >= 72163 && c <= 72164)
                    : c <= 72254)))
                : (c <= 72263 || (c < 72368
                  ? (c < 72349
                    ? (c >= 72272 && c <= 72345)
                    : c <= 72349)
                  : (c <= 72440 || (c < 72714
                    ? (c >= 72704 && c <= 72712)
                    : c <= 72758)))))))
            : (c <= 72768 || (c < 73056
              ? (c < 72968
                ? (c < 72850
                  ? (c < 72818
                    ? (c >= 72784 && c <= 72793)
                    : c <= 72847)
                  : (c <= 72871 || (c < 72960
                    ? (c >= 72873 && c <= 72886)
                    : c <= 72966)))
                : (c <= 72969 || (c < 73020
                  ? (c < 73018
                    ? (c >= 72971 && c <= 73014)
                    : c <= 73018)
                  : (c <= 73021 || (c < 73040
                    ? (c >= 73023 && c <= 73031)
                    : c <= 73049)))))
              : (c <= 73061 || (c < 73440
                ? (c < 73104
                  ? (c < 73066
                    ? (c >= 73063 && c <= 73064)
                    : c <= 73102)
                  : (c <= 73105 || (c < 73120
                    ? (c >= 73107 && c <= 73112)
                    : c <= 73129)))
                : (c <= 73462 || (c < 74752
                  ? (c < 73728
                    ? c == 73648
                    : c <= 74649)
                  : (c <= 74862 || (c < 77712
                    ? (c >= 74880 && c <= 75075)
                    : c <= 77808)))))))))
          : (c <= 78894 || (c < 110576
            ? (c < 93027
              ? (c < 92864
                ? (c < 92736
                  ? (c < 92160
                    ? (c >= 82944 && c <= 83526)
                    : c <= 92728)
                  : (c <= 92766 || (c < 92784
                    ? (c >= 92768 && c <= 92777)
                    : c <= 92862)))
                : (c <= 92873 || (c < 92928
                  ? (c < 92912
                    ? (c >= 92880 && c <= 92909)
                    : c <= 92916)
                  : (c <= 92982 || (c < 93008
                    ? (c >= 92992 && c <= 92995)
                    : c <= 93017)))))
              : (c <= 93047 || (c < 94176
                ? (c < 93952
                  ? (c < 93760
                    ? (c >= 93053 && c <= 93071)
                    : c <= 93823)
                  : (c <= 94026 || (c < 94095
                    ? (c >= 94031 && c <= 94087)
                    : c <= 94111)))
                : (c <= 94177 || (c < 94208
                  ? (c < 94192
                    ? (c >= 94179 && c <= 94180)
                    : c <= 94193)
                  : (c <= 100343 || (c < 101632
                    ? (c >= 100352 && c <= 101589)
                    : c <= 101640)))))))
            : (c <= 110579 || (c < 118528
              ? (c < 110960
                ? (c < 110592
                  ? (c < 110589
                    ? (c >= 110581 && c <= 110587)
                    : c <= 110590)
                  : (c <= 110882 || (c < 110948
                    ? (c >= 110928 && c <= 110930)
                    : c <= 110951)))
                : (c <= 111355 || (c < 113792
                  ? (c < 113776
                    ? (c >= 113664 && c <= 113770)
                    : c <= 113788)
                  : (c <= 113800 || (c < 113821
                    ? (c >= 113808 && c <= 113817)
                    : c <= 113822)))))
              : (c <= 118573 || (c < 119210
                ? (c < 119149
                  ? (c < 119141
                    ? (c >= 118576 && c <= 118598)
                    : c <= 119145)
                  : (c <= 119154 || (c < 119173
                    ? (c >= 119163 && c <= 119170)
                    : c <= 119179)))
                : (c <= 119213 || (c < 119894
                  ? (c < 119808
                    ? (c >= 119362 && c <= 119364)
                    : c <= 119892)
                  : (c <= 119964 || (c < 119970
                    ? (c >= 119966 && c <= 119967)
                    : c <= 119970)))))))))))
        : (c <= 119974 || (c < 124912
          ? (c < 120746
            ? (c < 120134
              ? (c < 120071
                ? (c < 119995
                  ? (c < 119982
                    ? (c >= 119977 && c <= 119980)
                    : c <= 119993)
                  : (c <= 119995 || (c < 120005
                    ? (c >= 119997 && c <= 120003)
                    : c <= 120069)))
                : (c <= 120074 || (c < 120094
                  ? (c < 120086
                    ? (c >= 120077 && c <= 120084)
                    : c <= 120092)
                  : (c <= 120121 || (c < 120128
                    ? (c >= 120123 && c <= 120126)
                    : c <= 120132)))))
              : (c <= 120134 || (c < 120572
                ? (c < 120488
                  ? (c < 120146
                    ? (c >= 120138 && c <= 120144)
                    : c <= 120485)
                  : (c <= 120512 || (c < 120540
                    ? (c >= 120514 && c <= 120538)
                    : c <= 120570)))
                : (c <= 120596 || (c < 120656
                  ? (c < 120630
                    ? (c >= 120598 && c <= 120628)
                    : c <= 120654)
                  : (c <= 120686 || (c < 120714
                    ? (c >= 120688 && c <= 120712)
                    : c <= 120744)))))))
            : (c <= 120770 || (c < 122907
              ? (c < 121476
                ? (c < 121344
                  ? (c < 120782
                    ? (c >= 120772 && c <= 120779)
                    : c <= 120831)
                  : (c <= 121398 || (c < 121461
                    ? (c >= 121403 && c <= 121452)
                    : c <= 121461)))
                : (c <= 121476 || (c < 122624
                  ? (c < 121505
                    ? (c >= 121499 && c <= 121503)
                    : c <= 121519)
                  : (c <= 122654 || (c < 122888
                    ? (c >= 122880 && c <= 122886)
                    : c <= 122904)))))
              : (c <= 122913 || (c < 123214
                ? (c < 123136
                  ? (c < 122918
                    ? (c >= 122915 && c <= 122916)
                    : c <= 122922)
                  : (c <= 123180 || (c < 123200
                    ? (c >= 123184 && c <= 123197)
                    : c <= 123209)))
                : (c <= 123214 || (c < 124896
                  ? (c < 123584
                    ? (c >= 123536 && c <= 123566)
                    : c <= 123641)
                  : (c <= 124902 || (c < 124909
                    ? (c >= 124904 && c <= 124907)
                    : c <= 124910)))))))))
          : (c <= 124926 || (c < 126557
            ? (c < 126521
              ? (c < 126469
                ? (c < 125184
                  ? (c < 125136
                    ? (c >= 124928 && c <= 125124)
                    : c <= 125142)
                  : (c <= 125259 || (c < 126464
                    ? (c >= 125264 && c <= 125273)
                    : c <= 126467)))
                : (c <= 126495 || (c < 126503
                  ? (c < 126500
                    ? (c >= 126497 && c <= 126498)
                    : c <= 126500)
                  : (c <= 126503 || (c < 126516
                    ? (c >= 126505 && c <= 126514)
                    : c <= 126519)))))
              : (c <= 126521 || (c < 126541
                ? (c < 126535
                  ? (c < 126530
                    ? c == 126523
                    : c <= 126530)
                  : (c <= 126535 || (c < 126539
                    ? c == 126537
                    : c <= 126539)))
                : (c <= 126543 || (c < 126551
                  ? (c < 126548
                    ? (c >= 126545 && c <= 126546)
                    : c <= 126548)
                  : (c <= 126551 || (c < 126555
                    ? c == 126553
                    : c <= 126555)))))))
            : (c <= 126557 || (c < 126629
              ? (c < 126580
                ? (c < 126564
                  ? (c < 126561
                    ? c == 126559
                    : c <= 126562)
                  : (c <= 126564 || (c < 126572
                    ? (c >= 126567 && c <= 126570)
                    : c <= 126578)))
                : (c <= 126583 || (c < 126592
                  ? (c < 126590
                    ? (c >= 126585 && c <= 126588)
                    : c <= 126590)
                  : (c <= 126601 || (c < 126625
                    ? (c >= 126603 && c <= 126619)
                    : c <= 126627)))))
              : (c <= 126633 || (c < 178208
                ? (c < 131072
                  ? (c < 130032
                    ? (c >= 126635 && c <= 126651)
                    : c <= 130041)
                  : (c <= 173791 || (c < 177984
                    ? (c >= 173824 && c <= 177976)
                    : c <= 178205)))
                : (c <= 183969 || (c < 196608
                  ? (c < 194560
                    ? (c >= 183984 && c <= 191456)
                    : c <= 195101)
                  : (c <= 201546 || (c >= 917760 && c <= 917999)))))))))))))))));
}

static bool ts_lex(TSLexer *lexer, TSStateId state) {
  START_LEXER();
  eof = lexer->eof(lexer);
  switch (state) {
    case 0:
      if (eof) ADVANCE(33);
      if (lookahead == '!') ADVANCE(68);
      if (lookahead == '%') ADVANCE(85);
      if (lookahead == '&') ADVANCE(60);
      if (lookahead == '(') ADVANCE(99);
      if (lookahead == ')') ADVANCE(100);
      if (lookahead == '*') ADVANCE(103);
      if (lookahead == '+') ADVANCE(92);
      if (lookahead == ',') ADVANCE(74);
      if (lookahead == '-') ADVANCE(87);
      if (lookahead == '.') ADVANCE(90);
      if (lookahead == '/') ADVANCE(66);
      if (lookahead == '0') ADVANCE(37);
      if (lookahead == ':') ADVANCE(73);
      if (lookahead == ';') ADVANCE(101);
      if (lookahead == '<') ADVANCE(82);
      if (lookahead == '=') ADVANCE(76);
      if (lookahead == '>') ADVANCE(80);
      if (lookahead == '@') ADVANCE(64);
      if (lookahead == '[') ADVANCE(69);
      if (lookahead == ']') ADVANCE(70);
      if (lookahead == '^') ADVANCE(107);
      if (lookahead == '_') ADVANCE(105);
      if (lookahead == '{') ADVANCE(71);
      if (lookahead == '|') ADVANCE(96);
      if (lookahead == '}') ADVANCE(72);
      if (lookahead == '~') ADVANCE(104);
      if (('1' <= lookahead && lookahead <= '9')) ADVANCE(39);
      if (('\t' <= lookahead && lookahead <= '\r') ||
          lookahead == ' ' ||
          lookahead == 133 ||
          lookahead == 8206 ||
          lookahead == 8207 ||
          lookahead == 8232 ||
          lookahead == 8233) ADVANCE(122);
      if (sym_ident_character_set_1(lookahead)) ADVANCE(116);
      END_STATE();
    case 1:
      if (lookahead == '!') ADVANCE(11);
      if (lookahead == '%') ADVANCE(84);
      if (lookahead == '&') ADVANCE(59);
      if (lookahead == '(') ADVANCE(99);
      if (lookahead == ')') ADVANCE(100);
      if (lookahead == '*') ADVANCE(102);
      if (lookahead == '+') ADVANCE(91);
      if (lookahead == ',') ADVANCE(74);
      if (lookahead == '-') ADVANCE(86);
      if (lookahead == '.') ADVANCE(89);
      if (lookahead == '/') ADVANCE(65);
      if (lookahead == ':') ADVANCE(73);
      if (lookahead == ';') ADVANCE(101);
      if (lookahead == '<') ADVANCE(82);
      if (lookahead == '=') ADVANCE(15);
      if (lookahead == '>') ADVANCE(80);
      if (lookahead == '[') ADVANCE(69);
      if (lookahead == ']') ADVANCE(70);
      if (lookahead == '^') ADVANCE(106);
      if (lookahead == '{') ADVANCE(71);
      if (lookahead == '|') ADVANCE(97);
      if (('\t' <= lookahead && lookahead <= '\r') ||
          lookahead == ' ' ||
          lookahead == 133 ||
          lookahead == 8206 ||
          lookahead == 8207 ||
          lookahead == 8232 ||
          lookahead == 8233) ADVANCE(122);
      END_STATE();
    case 2:
      if (lookahead == '%') ADVANCE(84);
      if (lookahead == '&') ADVANCE(58);
      if (lookahead == '(') ADVANCE(99);
      if (lookahead == ')') ADVANCE(100);
      if (lookahead == '*') ADVANCE(102);
      if (lookahead == '+') ADVANCE(91);
      if (lookahead == ',') ADVANCE(74);
      if (lookahead == '-') ADVANCE(86);
      if (lookahead == '.') ADVANCE(89);
      if (lookahead == '/') ADVANCE(65);
      if (lookahead == ':') ADVANCE(73);
      if (lookahead == ';') ADVANCE(101);
      if (lookahead == '>') ADVANCE(79);
      if (lookahead == '[') ADVANCE(69);
      if (lookahead == ']') ADVANCE(70);
      if (lookahead == '^') ADVANCE(106);
      if (lookahead == '{') ADVANCE(71);
      if (lookahead == '|') ADVANCE(94);
      if (('\t' <= lookahead && lookahead <= '\r') ||
          lookahead == ' ' ||
          lookahead == 133 ||
          lookahead == 8206 ||
          lookahead == 8207 ||
          lookahead == 8232 ||
          lookahead == 8233) ADVANCE(122);
      END_STATE();
    case 3:
      if (lookahead == '%') ADVANCE(12);
      if (lookahead == '&') ADVANCE(61);
      if (lookahead == ')') ADVANCE(100);
      if (lookahead == '*') ADVANCE(14);
      if (lookahead == '+') ADVANCE(5);
      if (lookahead == ',') ADVANCE(74);
      if (lookahead == '-') ADVANCE(7);
      if (lookahead == '.') ADVANCE(89);
      if (lookahead == '/') ADVANCE(10);
      if (lookahead == ':') ADVANCE(73);
      if (lookahead == ';') ADVANCE(101);
      if (lookahead == '=') ADVANCE(75);
      if (lookahead == '>') ADVANCE(79);
      if (lookahead == '[') ADVANCE(69);
      if (lookahead == ']') ADVANCE(70);
      if (lookahead == '^') ADVANCE(107);
      if (lookahead == '{') ADVANCE(71);
      if (lookahead == '|') ADVANCE(95);
      if (('\t' <= lookahead && lookahead <= '\r') ||
          lookahead == ' ' ||
          lookahead == 133 ||
          lookahead == 8206 ||
          lookahead == 8207 ||
          lookahead == 8232 ||
          lookahead == 8233) ADVANCE(122);
      END_STATE();
    case 4:
      if (lookahead == '%') ADVANCE(12);
      if (lookahead == '&') ADVANCE(13);
      if (lookahead == '(') ADVANCE(99);
      if (lookahead == ')') ADVANCE(100);
      if (lookahead == '*') ADVANCE(14);
      if (lookahead == '+') ADVANCE(5);
      if (lookahead == '-') ADVANCE(6);
      if (lookahead == '.') ADVANCE(89);
      if (lookahead == '/') ADVANCE(10);
      if (lookahead == '=') ADVANCE(75);
      if (lookahead == '[') ADVANCE(69);
      if (lookahead == '^') ADVANCE(16);
      if (lookahead == '|') ADVANCE(17);
      if (('\t' <= lookahead && lookahead <= '\r') ||
          lookahead == ' ' ||
          lookahead == 133 ||
          lookahead == 8206 ||
          lookahead == 8207 ||
          lookahead == 8232 ||
          lookahead == 8233) ADVANCE(122);
      END_STATE();
    case 5:
      if (lookahead == '+') ADVANCE(93);
      if (lookahead == '=') ADVANCE(108);
      END_STATE();
    case 6:
      if (lookahead == '-') ADVANCE(88);
      if (lookahead == '=') ADVANCE(109);
      END_STATE();
    case 7:
      if (lookahead == '-') ADVANCE(88);
      if (lookahead == '=') ADVANCE(109);
      if (lookahead == '>') ADVANCE(63);
      END_STATE();
    case 8:
      if (lookahead == '.') ADVANCE(44);
      if (lookahead == 'E' ||
          lookahead == 'e') ADVANCE(18);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(8);
      END_STATE();
    case 9:
      if (lookahead == '.') ADVANCE(31);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(35);
      END_STATE();
    case 10:
      if (lookahead == '/') ADVANCE(117);
      if (lookahead == '=') ADVANCE(111);
      END_STATE();
    case 11:
      if (lookahead == '=') ADVANCE(78);
      END_STATE();
    case 12:
      if (lookahead == '=') ADVANCE(112);
      END_STATE();
    case 13:
      if (lookahead == '=') ADVANCE(113);
      END_STATE();
    case 14:
      if (lookahead == '=') ADVANCE(110);
      END_STATE();
    case 15:
      if (lookahead == '=') ADVANCE(77);
      END_STATE();
    case 16:
      if (lookahead == '=') ADVANCE(115);
      END_STATE();
    case 17:
      if (lookahead == '=') ADVANCE(114);
      END_STATE();
    case 18:
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(25);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(47);
      END_STATE();
    case 19:
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(26);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(42);
      END_STATE();
    case 20:
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(27);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(45);
      END_STATE();
    case 21:
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(28);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(57);
      END_STATE();
    case 22:
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(29);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(52);
      END_STATE();
    case 23:
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(30);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(55);
      END_STATE();
    case 24:
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(41);
      END_STATE();
    case 25:
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(47);
      END_STATE();
    case 26:
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(42);
      END_STATE();
    case 27:
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(45);
      END_STATE();
    case 28:
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(57);
      END_STATE();
    case 29:
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(52);
      END_STATE();
    case 30:
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(55);
      END_STATE();
    case 31:
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(51);
      END_STATE();
    case 32:
      if (eof) ADVANCE(33);
      if (lookahead == '!') ADVANCE(67);
      if (lookahead == '%') ADVANCE(84);
      if (lookahead == '&') ADVANCE(58);
      if (lookahead == '(') ADVANCE(99);
      if (lookahead == ')') ADVANCE(100);
      if (lookahead == '*') ADVANCE(102);
      if (lookahead == '+') ADVANCE(91);
      if (lookahead == ',') ADVANCE(74);
      if (lookahead == '-') ADVANCE(86);
      if (lookahead == '.') ADVANCE(24);
      if (lookahead == '/') ADVANCE(65);
      if (lookahead == '0') ADVANCE(37);
      if (lookahead == ':') ADVANCE(73);
      if (lookahead == ';') ADVANCE(101);
      if (lookahead == '=') ADVANCE(75);
      if (lookahead == '>') ADVANCE(79);
      if (lookahead == '@') ADVANCE(64);
      if (lookahead == ']') ADVANCE(70);
      if (lookahead == '^') ADVANCE(106);
      if (lookahead == '_') ADVANCE(105);
      if (lookahead == '{') ADVANCE(71);
      if (lookahead == '|') ADVANCE(94);
      if (lookahead == '}') ADVANCE(72);
      if (lookahead == '~') ADVANCE(104);
      if (('1' <= lookahead && lookahead <= '9')) ADVANCE(39);
      if (('\t' <= lookahead && lookahead <= '\r') ||
          lookahead == ' ' ||
          lookahead == 133 ||
          lookahead == 8206 ||
          lookahead == 8207 ||
          lookahead == 8232 ||
          lookahead == 8233) ADVANCE(122);
      if (sym_ident_character_set_1(lookahead)) ADVANCE(116);
      END_STATE();
    case 33:
      ACCEPT_TOKEN(ts_builtin_sym_end);
      END_STATE();
    case 34:
      ACCEPT_TOKEN(aux_sym_int_literal_token1);
      END_STATE();
    case 35:
      ACCEPT_TOKEN(aux_sym_int_literal_token1);
      if (lookahead == '.') ADVANCE(54);
      if (lookahead == 'P' ||
          lookahead == 'p') ADVANCE(21);
      if (lookahead == 'i' ||
          lookahead == 'u') ADVANCE(34);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(35);
      END_STATE();
    case 36:
      ACCEPT_TOKEN(aux_sym_int_literal_token2);
      END_STATE();
    case 37:
      ACCEPT_TOKEN(aux_sym_int_literal_token2);
      if (lookahead == '.') ADVANCE(44);
      if (lookahead == 'E' ||
          lookahead == 'e') ADVANCE(18);
      if (lookahead == 'X' ||
          lookahead == 'x') ADVANCE(9);
      if (lookahead == 'f' ||
          lookahead == 'h') ADVANCE(48);
      if (lookahead == 'i' ||
          lookahead == 'u') ADVANCE(36);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(8);
      END_STATE();
    case 38:
      ACCEPT_TOKEN(aux_sym_int_literal_token3);
      END_STATE();
    case 39:
      ACCEPT_TOKEN(aux_sym_int_literal_token3);
      if (lookahead == '.') ADVANCE(44);
      if (lookahead == 'E' ||
          lookahead == 'e') ADVANCE(18);
      if (lookahead == 'f' ||
          lookahead == 'h') ADVANCE(49);
      if (lookahead == 'i' ||
          lookahead == 'u') ADVANCE(38);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(39);
      END_STATE();
    case 40:
      ACCEPT_TOKEN(aux_sym_decimal_float_literal_token1);
      END_STATE();
    case 41:
      ACCEPT_TOKEN(aux_sym_decimal_float_literal_token1);
      if (lookahead == 'E' ||
          lookahead == 'e') ADVANCE(19);
      if (lookahead == 'f' ||
          lookahead == 'h') ADVANCE(40);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(41);
      END_STATE();
    case 42:
      ACCEPT_TOKEN(aux_sym_decimal_float_literal_token1);
      if (lookahead == 'f' ||
          lookahead == 'h') ADVANCE(40);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(42);
      END_STATE();
    case 43:
      ACCEPT_TOKEN(aux_sym_decimal_float_literal_token2);
      END_STATE();
    case 44:
      ACCEPT_TOKEN(aux_sym_decimal_float_literal_token2);
      if (lookahead == 'E' ||
          lookahead == 'e') ADVANCE(20);
      if (lookahead == 'f' ||
          lookahead == 'h') ADVANCE(43);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(41);
      END_STATE();
    case 45:
      ACCEPT_TOKEN(aux_sym_decimal_float_literal_token2);
      if (lookahead == 'f' ||
          lookahead == 'h') ADVANCE(43);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(45);
      END_STATE();
    case 46:
      ACCEPT_TOKEN(aux_sym_decimal_float_literal_token3);
      END_STATE();
    case 47:
      ACCEPT_TOKEN(aux_sym_decimal_float_literal_token3);
      if (lookahead == 'f' ||
          lookahead == 'h') ADVANCE(46);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(47);
      END_STATE();
    case 48:
      ACCEPT_TOKEN(aux_sym_decimal_float_literal_token4);
      END_STATE();
    case 49:
      ACCEPT_TOKEN(aux_sym_decimal_float_literal_token5);
      END_STATE();
    case 50:
      ACCEPT_TOKEN(aux_sym_hex_float_literal_token1);
      END_STATE();
    case 51:
      ACCEPT_TOKEN(aux_sym_hex_float_literal_token1);
      if (lookahead == 'P' ||
          lookahead == 'p') ADVANCE(22);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(51);
      END_STATE();
    case 52:
      ACCEPT_TOKEN(aux_sym_hex_float_literal_token1);
      if (lookahead == 'f' ||
          lookahead == 'h') ADVANCE(50);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(52);
      END_STATE();
    case 53:
      ACCEPT_TOKEN(aux_sym_hex_float_literal_token2);
      END_STATE();
    case 54:
      ACCEPT_TOKEN(aux_sym_hex_float_literal_token2);
      if (lookahead == 'P' ||
          lookahead == 'p') ADVANCE(23);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(51);
      END_STATE();
    case 55:
      ACCEPT_TOKEN(aux_sym_hex_float_literal_token2);
      if (lookahead == 'f' ||
          lookahead == 'h') ADVANCE(53);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(55);
      END_STATE();
    case 56:
      ACCEPT_TOKEN(aux_sym_hex_float_literal_token3);
      END_STATE();
    case 57:
      ACCEPT_TOKEN(aux_sym_hex_float_literal_token3);
      if (lookahead == 'f' ||
          lookahead == 'h') ADVANCE(56);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(57);
      END_STATE();
    case 58:
      ACCEPT_TOKEN(sym_and);
      END_STATE();
    case 59:
      ACCEPT_TOKEN(sym_and);
      if (lookahead == '&') ADVANCE(62);
      END_STATE();
    case 60:
      ACCEPT_TOKEN(sym_and);
      if (lookahead == '&') ADVANCE(62);
      if (lookahead == '=') ADVANCE(113);
      END_STATE();
    case 61:
      ACCEPT_TOKEN(sym_and);
      if (lookahead == '=') ADVANCE(113);
      END_STATE();
    case 62:
      ACCEPT_TOKEN(sym_and_and);
      END_STATE();
    case 63:
      ACCEPT_TOKEN(sym_arrow);
      END_STATE();
    case 64:
      ACCEPT_TOKEN(sym_attr);
      END_STATE();
    case 65:
      ACCEPT_TOKEN(sym_forward_slash);
      if (lookahead == '/') ADVANCE(117);
      END_STATE();
    case 66:
      ACCEPT_TOKEN(sym_forward_slash);
      if (lookahead == '/') ADVANCE(117);
      if (lookahead == '=') ADVANCE(111);
      END_STATE();
    case 67:
      ACCEPT_TOKEN(sym_bang);
      END_STATE();
    case 68:
      ACCEPT_TOKEN(sym_bang);
      if (lookahead == '=') ADVANCE(78);
      END_STATE();
    case 69:
      ACCEPT_TOKEN(sym_bracket_left);
      END_STATE();
    case 70:
      ACCEPT_TOKEN(sym_bracket_right);
      END_STATE();
    case 71:
      ACCEPT_TOKEN(sym_brace_left);
      END_STATE();
    case 72:
      ACCEPT_TOKEN(sym_brace_right);
      END_STATE();
    case 73:
      ACCEPT_TOKEN(sym_colon);
      END_STATE();
    case 74:
      ACCEPT_TOKEN(sym_comma);
      END_STATE();
    case 75:
      ACCEPT_TOKEN(sym_equal);
      END_STATE();
    case 76:
      ACCEPT_TOKEN(sym_equal);
      if (lookahead == '=') ADVANCE(77);
      END_STATE();
    case 77:
      ACCEPT_TOKEN(sym_equal_equal);
      END_STATE();
    case 78:
      ACCEPT_TOKEN(sym_not_equal);
      END_STATE();
    case 79:
      ACCEPT_TOKEN(sym_greater_than);
      END_STATE();
    case 80:
      ACCEPT_TOKEN(sym_greater_than);
      if (lookahead == '=') ADVANCE(81);
      END_STATE();
    case 81:
      ACCEPT_TOKEN(sym_greater_than_equal);
      END_STATE();
    case 82:
      ACCEPT_TOKEN(sym_less_than);
      if (lookahead == '=') ADVANCE(83);
      END_STATE();
    case 83:
      ACCEPT_TOKEN(sym_less_than_equal);
      END_STATE();
    case 84:
      ACCEPT_TOKEN(sym_modulo);
      END_STATE();
    case 85:
      ACCEPT_TOKEN(sym_modulo);
      if (lookahead == '=') ADVANCE(112);
      END_STATE();
    case 86:
      ACCEPT_TOKEN(sym_minus);
      END_STATE();
    case 87:
      ACCEPT_TOKEN(sym_minus);
      if (lookahead == '=') ADVANCE(109);
      if (lookahead == '>') ADVANCE(63);
      END_STATE();
    case 88:
      ACCEPT_TOKEN(sym_minus_minus);
      END_STATE();
    case 89:
      ACCEPT_TOKEN(sym_period);
      END_STATE();
    case 90:
      ACCEPT_TOKEN(sym_period);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(41);
      END_STATE();
    case 91:
      ACCEPT_TOKEN(sym_plus);
      END_STATE();
    case 92:
      ACCEPT_TOKEN(sym_plus);
      if (lookahead == '+') ADVANCE(93);
      if (lookahead == '=') ADVANCE(108);
      END_STATE();
    case 93:
      ACCEPT_TOKEN(sym_plus_plus);
      END_STATE();
    case 94:
      ACCEPT_TOKEN(sym_or);
      END_STATE();
    case 95:
      ACCEPT_TOKEN(sym_or);
      if (lookahead == '=') ADVANCE(114);
      END_STATE();
    case 96:
      ACCEPT_TOKEN(sym_or);
      if (lookahead == '=') ADVANCE(114);
      if (lookahead == '|') ADVANCE(98);
      END_STATE();
    case 97:
      ACCEPT_TOKEN(sym_or);
      if (lookahead == '|') ADVANCE(98);
      END_STATE();
    case 98:
      ACCEPT_TOKEN(sym_or_or);
      END_STATE();
    case 99:
      ACCEPT_TOKEN(sym_paren_left);
      END_STATE();
    case 100:
      ACCEPT_TOKEN(sym_paren_right);
      END_STATE();
    case 101:
      ACCEPT_TOKEN(sym_semicolon);
      END_STATE();
    case 102:
      ACCEPT_TOKEN(sym_star);
      END_STATE();
    case 103:
      ACCEPT_TOKEN(sym_star);
      if (lookahead == '=') ADVANCE(110);
      END_STATE();
    case 104:
      ACCEPT_TOKEN(sym_tilde);
      END_STATE();
    case 105:
      ACCEPT_TOKEN(sym_underscore);
      if (sym_ident_character_set_2(lookahead)) ADVANCE(116);
      END_STATE();
    case 106:
      ACCEPT_TOKEN(sym_xor);
      END_STATE();
    case 107:
      ACCEPT_TOKEN(sym_xor);
      if (lookahead == '=') ADVANCE(115);
      END_STATE();
    case 108:
      ACCEPT_TOKEN(sym_plus_equal);
      END_STATE();
    case 109:
      ACCEPT_TOKEN(sym_minus_equal);
      END_STATE();
    case 110:
      ACCEPT_TOKEN(sym_times_equal);
      END_STATE();
    case 111:
      ACCEPT_TOKEN(sym_division_equal);
      END_STATE();
    case 112:
      ACCEPT_TOKEN(sym_modulo_equal);
      END_STATE();
    case 113:
      ACCEPT_TOKEN(sym_and_equal);
      END_STATE();
    case 114:
      ACCEPT_TOKEN(sym_or_equal);
      END_STATE();
    case 115:
      ACCEPT_TOKEN(sym_xor_equal);
      END_STATE();
    case 116:
      ACCEPT_TOKEN(sym_ident);
      if (sym_ident_character_set_2(lookahead)) ADVANCE(116);
      END_STATE();
    case 117:
      ACCEPT_TOKEN(anon_sym_SLASH_SLASH);
      END_STATE();
    case 118:
      ACCEPT_TOKEN(anon_sym_SLASH_SLASH);
      if (lookahead != 0 &&
          lookahead != '\n') ADVANCE(121);
      END_STATE();
    case 119:
      ACCEPT_TOKEN(aux_sym__comment_token1);
      if (lookahead == '\n') ADVANCE(122);
      if (lookahead == '/') ADVANCE(120);
      if (('\t' <= lookahead && lookahead <= '\r') ||
          lookahead == ' ' ||
          lookahead == 133 ||
          lookahead == 8206 ||
          lookahead == 8207 ||
          lookahead == 8232 ||
          lookahead == 8233) ADVANCE(121);
      if (lookahead != 0) ADVANCE(121);
      END_STATE();
    case 120:
      ACCEPT_TOKEN(aux_sym__comment_token1);
      if (lookahead == '/') ADVANCE(118);
      if (lookahead != 0 &&
          lookahead != '\n') ADVANCE(121);
      END_STATE();
    case 121:
      ACCEPT_TOKEN(aux_sym__comment_token1);
      if (lookahead != 0 &&
          lookahead != '\n') ADVANCE(121);
      END_STATE();
    case 122:
      ACCEPT_TOKEN(sym__blankspace);
      END_STATE();
    default:
      return false;
  }
}

static bool ts_lex_keywords(TSLexer *lexer, TSStateId state) {
  START_LEXER();
  eof = lexer->eof(lexer);
  switch (state) {
    case 0:
      if (lookahead == 'A') ADVANCE(1);
      if (lookahead == 'B') ADVANCE(2);
      if (lookahead == 'C') ADVANCE(3);
      if (lookahead == 'D') ADVANCE(4);
      if (lookahead == 'G') ADVANCE(5);
      if (lookahead == 'H') ADVANCE(6);
      if (lookahead == 'I') ADVANCE(7);
      if (lookahead == 'L') ADVANCE(8);
      if (lookahead == 'N') ADVANCE(9);
      if (lookahead == 'O') ADVANCE(10);
      if (lookahead == 'P') ADVANCE(11);
      if (lookahead == 'R') ADVANCE(12);
      if (lookahead == 'S') ADVANCE(13);
      if (lookahead == 'T') ADVANCE(14);
      if (lookahead == 'V') ADVANCE(15);
      if (lookahead == 'a') ADVANCE(16);
      if (lookahead == 'b') ADVANCE(17);
      if (lookahead == 'c') ADVANCE(18);
      if (lookahead == 'd') ADVANCE(19);
      if (lookahead == 'e') ADVANCE(20);
      if (lookahead == 'f') ADVANCE(21);
      if (lookahead == 'g') ADVANCE(22);
      if (lookahead == 'h') ADVANCE(23);
      if (lookahead == 'i') ADVANCE(24);
      if (lookahead == 'l') ADVANCE(25);
      if (lookahead == 'm') ADVANCE(26);
      if (lookahead == 'n') ADVANCE(27);
      if (lookahead == 'o') ADVANCE(28);
      if (lookahead == 'p') ADVANCE(29);
      if (lookahead == 'r') ADVANCE(30);
      if (lookahead == 's') ADVANCE(31);
      if (lookahead == 't') ADVANCE(32);
      if (lookahead == 'u') ADVANCE(33);
      if (lookahead == 'v') ADVANCE(34);
      if (lookahead == 'w') ADVANCE(35);
      if (lookahead == 'y') ADVANCE(36);
      END_STATE();
    case 1:
      if (lookahead == 'p') ADVANCE(37);
      END_STATE();
    case 2:
      if (lookahead == 'l') ADVANCE(38);
      if (lookahead == 'u') ADVANCE(39);
      if (lookahead == 'y') ADVANCE(40);
      END_STATE();
    case 3:
      if (lookahead == 'o') ADVANCE(41);
      END_STATE();
    case 4:
      if (lookahead == 'e') ADVANCE(42);
      if (lookahead == 'o') ADVANCE(43);
      END_STATE();
    case 5:
      if (lookahead == 'e') ADVANCE(44);
      END_STATE();
    case 6:
      if (lookahead == 'u') ADVANCE(45);
      END_STATE();
    case 7:
      if (lookahead == 'n') ADVANCE(46);
      END_STATE();
    case 8:
      if (lookahead == 'i') ADVANCE(47);
      END_STATE();
    case 9:
      if (lookahead == 'U') ADVANCE(48);
      END_STATE();
    case 10:
      if (lookahead == 'u') ADVANCE(49);
      END_STATE();
    case 11:
      if (lookahead == 'i') ADVANCE(50);
      if (lookahead == 'o') ADVANCE(51);
      END_STATE();
    case 12:
      if (lookahead == 'W') ADVANCE(52);
      if (lookahead == 'a') ADVANCE(53);
      if (lookahead == 'e') ADVANCE(54);
      END_STATE();
    case 13:
      if (lookahead == 'a') ADVANCE(55);
      if (lookahead == 'e') ADVANCE(56);
      if (lookahead == 't') ADVANCE(57);
      END_STATE();
    case 14:
      if (lookahead == 'e') ADVANCE(58);
      if (lookahead == 'r') ADVANCE(59);
      END_STATE();
    case 15:
      if (lookahead == 'e') ADVANCE(60);
      END_STATE();
    case 16:
      if (lookahead == 'b') ADVANCE(61);
      if (lookahead == 'c') ADVANCE(62);
      if (lookahead == 'l') ADVANCE(63);
      if (lookahead == 'r') ADVANCE(64);
      if (lookahead == 's') ADVANCE(65);
      if (lookahead == 't') ADVANCE(66);
      if (lookahead == 'u') ADVANCE(67);
      if (lookahead == 'w') ADVANCE(68);
      END_STATE();
    case 17:
      if (lookahead == 'e') ADVANCE(69);
      if (lookahead == 'f') ADVANCE(70);
      if (lookahead == 'i') ADVANCE(71);
      if (lookahead == 'o') ADVANCE(72);
      if (lookahead == 'r') ADVANCE(73);
      END_STATE();
    case 18:
      if (lookahead == 'a') ADVANCE(74);
      if (lookahead == 'b') ADVANCE(75);
      if (lookahead == 'e') ADVANCE(76);
      if (lookahead == 'h') ADVANCE(77);
      if (lookahead == 'l') ADVANCE(78);
      if (lookahead == 'o') ADVANCE(79);
      if (lookahead == 'r') ADVANCE(80);
      END_STATE();
    case 19:
      if (lookahead == 'e') ADVANCE(81);
      if (lookahead == 'i') ADVANCE(82);
      if (lookahead == 'o') ADVANCE(83);
      if (lookahead == 'w') ADVANCE(84);
      if (lookahead == 'y') ADVANCE(85);
      END_STATE();
    case 20:
      if (lookahead == 'l') ADVANCE(86);
      if (lookahead == 'n') ADVANCE(87);
      if (lookahead == 'x') ADVANCE(88);
      END_STATE();
    case 21:
      if (lookahead == '1') ADVANCE(89);
      if (lookahead == '3') ADVANCE(90);
      if (lookahead == '6') ADVANCE(91);
      if (lookahead == 'a') ADVANCE(92);
      if (lookahead == 'i') ADVANCE(93);
      if (lookahead == 'l') ADVANCE(94);
      if (lookahead == 'n') ADVANCE(95);
      if (lookahead == 'o') ADVANCE(96);
      if (lookahead == 'r') ADVANCE(97);
      if (lookahead == 'u') ADVANCE(98);
      if (lookahead == 'v') ADVANCE(99);
      if (lookahead == 'x') ADVANCE(100);
      END_STATE();
    case 22:
      if (lookahead == 'e') ADVANCE(101);
      if (lookahead == 'o') ADVANCE(102);
      if (lookahead == 'r') ADVANCE(103);
      END_STATE();
    case 23:
      if (lookahead == 'a') ADVANCE(104);
      if (lookahead == 'i') ADVANCE(105);
      if (lookahead == 'v') ADVANCE(106);
      END_STATE();
    case 24:
      if (lookahead == '1') ADVANCE(107);
      if (lookahead == '3') ADVANCE(108);
      if (lookahead == '6') ADVANCE(109);
      if (lookahead == '8') ADVANCE(110);
      if (lookahead == 'f') ADVANCE(111);
      if (lookahead == 'i') ADVANCE(112);
      if (lookahead == 'm') ADVANCE(113);
      if (lookahead == 'n') ADVANCE(114);
      if (lookahead == 's') ADVANCE(115);
      if (lookahead == 't') ADVANCE(116);
      END_STATE();
    case 25:
      if (lookahead == 'a') ADVANCE(117);
      if (lookahead == 'e') ADVANCE(118);
      if (lookahead == 'i') ADVANCE(119);
      if (lookahead == 'o') ADVANCE(120);
      END_STATE();
    case 26:
      if (lookahead == 'a') ADVANCE(121);
      if (lookahead == 'e') ADVANCE(122);
      if (lookahead == 'o') ADVANCE(123);
      if (lookahead == 'u') ADVANCE(124);
      END_STATE();
    case 27:
      if (lookahead == 'a') ADVANCE(125);
      if (lookahead == 'e') ADVANCE(126);
      if (lookahead == 'i') ADVANCE(127);
      if (lookahead == 'o') ADVANCE(128);
      if (lookahead == 'u') ADVANCE(129);
      END_STATE();
    case 28:
      if (lookahead == 'f') ADVANCE(130);
      if (lookahead == 'p') ADVANCE(131);
      if (lookahead == 'v') ADVANCE(132);
      END_STATE();
    case 29:
      if (lookahead == 'a') ADVANCE(133);
      if (lookahead == 'i') ADVANCE(134);
      if (lookahead == 'o') ADVANCE(135);
      if (lookahead == 'r') ADVANCE(136);
      if (lookahead == 't') ADVANCE(137);
      if (lookahead == 'u') ADVANCE(138);
      END_STATE();
    case 30:
      if (lookahead == '3') ADVANCE(139);
      if (lookahead == 'e') ADVANCE(140);
      if (lookahead == 'g') ADVANCE(141);
      if (lookahead == 'o') ADVANCE(142);
      END_STATE();
    case 31:
      if (lookahead == 'a') ADVANCE(143);
      if (lookahead == 'e') ADVANCE(144);
      if (lookahead == 'h') ADVANCE(145);
      if (lookahead == 'i') ADVANCE(146);
      if (lookahead == 'm') ADVANCE(147);
      if (lookahead == 'n') ADVANCE(148);
      if (lookahead == 't') ADVANCE(149);
      if (lookahead == 'u') ADVANCE(150);
      if (lookahead == 'w') ADVANCE(151);
      END_STATE();
    case 32:
      if (lookahead == 'a') ADVANCE(152);
      if (lookahead == 'b') ADVANCE(153);
      if (lookahead == 'e') ADVANCE(154);
      if (lookahead == 'h') ADVANCE(155);
      if (lookahead == 'r') ADVANCE(156);
      if (lookahead == 'y') ADVANCE(157);
      END_STATE();
    case 33:
      if (lookahead == '1') ADVANCE(158);
      if (lookahead == '3') ADVANCE(159);
      if (lookahead == '6') ADVANCE(160);
      if (lookahead == '8') ADVANCE(161);
      if (lookahead == 'i') ADVANCE(162);
      if (lookahead == 'n') ADVANCE(163);
      if (lookahead == 's') ADVANCE(164);
      if (lookahead == 't') ADVANCE(165);
      END_STATE();
    case 34:
      if (lookahead == 'a') ADVANCE(166);
      if (lookahead == 'e') ADVANCE(167);
      if (lookahead == 'i') ADVANCE(168);
      if (lookahead == 'o') ADVANCE(169);
      END_STATE();
    case 35:
      if (lookahead == 'c') ADVANCE(170);
      if (lookahead == 'g') ADVANCE(171);
      if (lookahead == 'h') ADVANCE(172);
      if (lookahead == 'i') ADVANCE(173);
      if (lookahead == 'o') ADVANCE(174);
      if (lookahead == 'r') ADVANCE(175);
      END_STATE();
    case 36:
      if (lookahead == 'i') ADVANCE(176);
      END_STATE();
    case 37:
      if (lookahead == 'p') ADVANCE(177);
      END_STATE();
    case 38:
      if (lookahead == 'e') ADVANCE(178);
      END_STATE();
    case 39:
      if (lookahead == 'f') ADVANCE(179);
      END_STATE();
    case 40:
      if (lookahead == 't') ADVANCE(180);
      END_STATE();
    case 41:
      if (lookahead == 'm') ADVANCE(181);
      if (lookahead == 'n') ADVANCE(182);
      END_STATE();
    case 42:
      if (lookahead == 'p') ADVANCE(183);
      END_STATE();
    case 43:
      if (lookahead == 'm') ADVANCE(184);
      END_STATE();
    case 44:
      if (lookahead == 'o') ADVANCE(185);
      END_STATE();
    case 45:
      if (lookahead == 'l') ADVANCE(186);
      END_STATE();
    case 46:
      if (lookahead == 'p') ADVANCE(187);
      END_STATE();
    case 47:
      if (lookahead == 'n') ADVANCE(188);
      END_STATE();
    case 48:
      if (lookahead == 'L') ADVANCE(189);
      END_STATE();
    case 49:
      if (lookahead == 't') ADVANCE(190);
      END_STATE();
    case 50:
      if (lookahead == 'x') ADVANCE(191);
      END_STATE();
    case 51:
      if (lookahead == 'i') ADVANCE(192);
      END_STATE();
    case 52:
      if (lookahead == 'B') ADVANCE(193);
      if (lookahead == 'S') ADVANCE(194);
      if (lookahead == 'T') ADVANCE(195);
      END_STATE();
    case 53:
      if (lookahead == 's') ADVANCE(196);
      END_STATE();
    case 54:
      if (lookahead == 'n') ADVANCE(197);
      END_STATE();
    case 55:
      if (lookahead == 'm') ADVANCE(198);
      END_STATE();
    case 56:
      if (lookahead == 'l') ADVANCE(199);
      END_STATE();
    case 57:
      if (lookahead == 'r') ADVANCE(200);
      END_STATE();
    case 58:
      if (lookahead == 'x') ADVANCE(201);
      END_STATE();
    case 59:
      if (lookahead == 'i') ADVANCE(202);
      END_STATE();
    case 60:
      if (lookahead == 'r') ADVANCE(203);
      END_STATE();
    case 61:
      if (lookahead == 's') ADVANCE(204);
      END_STATE();
    case 62:
      if (lookahead == 't') ADVANCE(205);
      END_STATE();
    case 63:
      if (lookahead == 'i') ADVANCE(206);
      END_STATE();
    case 64:
      if (lookahead == 'r') ADVANCE(207);
      END_STATE();
    case 65:
      ACCEPT_TOKEN(anon_sym_as);
      if (lookahead == 'm') ADVANCE(208);
      if (lookahead == 'y') ADVANCE(209);
      END_STATE();
    case 66:
      if (lookahead == 'o') ADVANCE(210);
      if (lookahead == 't') ADVANCE(211);
      END_STATE();
    case 67:
      if (lookahead == 't') ADVANCE(212);
      END_STATE();
    case 68:
      if (lookahead == 'a') ADVANCE(213);
      END_STATE();
    case 69:
      if (lookahead == 'c') ADVANCE(214);
      END_STATE();
    case 70:
      if (lookahead == '1') ADVANCE(215);
      END_STATE();
    case 71:
      if (lookahead == 'n') ADVANCE(216);
      if (lookahead == 't') ADVANCE(217);
      END_STATE();
    case 72:
      if (lookahead == 'o') ADVANCE(218);
      END_STATE();
    case 73:
      if (lookahead == 'e') ADVANCE(219);
      END_STATE();
    case 74:
      if (lookahead == 's') ADVANCE(220);
      if (lookahead == 't') ADVANCE(221);
      END_STATE();
    case 75:
      if (lookahead == 'u') ADVANCE(222);
      END_STATE();
    case 76:
      if (lookahead == 'n') ADVANCE(223);
      END_STATE();
    case 77:
      if (lookahead == 'a') ADVANCE(224);
      END_STATE();
    case 78:
      if (lookahead == 'a') ADVANCE(225);
      END_STATE();
    case 79:
      if (lookahead == '_') ADVANCE(226);
      if (lookahead == 'h') ADVANCE(227);
      if (lookahead == 'l') ADVANCE(228);
      if (lookahead == 'm') ADVANCE(229);
      if (lookahead == 'n') ADVANCE(230);
      END_STATE();
    case 80:
      if (lookahead == 'a') ADVANCE(231);
      END_STATE();
    case 81:
      if (lookahead == 'b') ADVANCE(232);
      if (lookahead == 'c') ADVANCE(233);
      if (lookahead == 'f') ADVANCE(234);
      if (lookahead == 'l') ADVANCE(235);
      if (lookahead == 'm') ADVANCE(236);
      END_STATE();
    case 82:
      if (lookahead == 's') ADVANCE(237);
      END_STATE();
    case 83:
      ACCEPT_TOKEN(anon_sym_do);
      END_STATE();
    case 84:
      if (lookahead == 'o') ADVANCE(238);
      END_STATE();
    case 85:
      if (lookahead == 'n') ADVANCE(239);
      END_STATE();
    case 86:
      if (lookahead == 's') ADVANCE(240);
      END_STATE();
    case 87:
      if (lookahead == 'a') ADVANCE(241);
      if (lookahead == 'u') ADVANCE(242);
      END_STATE();
    case 88:
      if (lookahead == 'p') ADVANCE(243);
      if (lookahead == 't') ADVANCE(244);
      END_STATE();
    case 89:
      if (lookahead == '6') ADVANCE(245);
      END_STATE();
    case 90:
      if (lookahead == '2') ADVANCE(246);
      END_STATE();
    case 91:
      if (lookahead == '4') ADVANCE(247);
      END_STATE();
    case 92:
      if (lookahead == 'l') ADVANCE(248);
      END_STATE();
    case 93:
      if (lookahead == 'l') ADVANCE(249);
      if (lookahead == 'n') ADVANCE(250);
      if (lookahead == 'x') ADVANCE(251);
      END_STATE();
    case 94:
      if (lookahead == 'a') ADVANCE(252);
      END_STATE();
    case 95:
      ACCEPT_TOKEN(sym_fn);
      END_STATE();
    case 96:
      if (lookahead == 'r') ADVANCE(253);
      END_STATE();
    case 97:
      if (lookahead == 'i') ADVANCE(254);
      if (lookahead == 'o') ADVANCE(255);
      END_STATE();
    case 98:
      if (lookahead == 'n') ADVANCE(256);
      END_STATE();
    case 99:
      if (lookahead == 'e') ADVANCE(257);
      END_STATE();
    case 100:
      if (lookahead == 'g') ADVANCE(258);
      END_STATE();
    case 101:
      if (lookahead == 't') ADVANCE(259);
      END_STATE();
    case 102:
      if (lookahead == 't') ADVANCE(260);
      END_STATE();
    case 103:
      if (lookahead == 'o') ADVANCE(261);
      END_STATE();
    case 104:
      if (lookahead == 'n') ADVANCE(262);
      END_STATE();
    case 105:
      if (lookahead == 'g') ADVANCE(263);
      END_STATE();
    case 106:
      if (lookahead == 'e') ADVANCE(264);
      END_STATE();
    case 107:
      if (lookahead == '6') ADVANCE(265);
      END_STATE();
    case 108:
      if (lookahead == '2') ADVANCE(266);
      END_STATE();
    case 109:
      if (lookahead == '4') ADVANCE(267);
      END_STATE();
    case 110:
      ACCEPT_TOKEN(anon_sym_i8);
      END_STATE();
    case 111:
      ACCEPT_TOKEN(sym_if);
      END_STATE();
    case 112:
      if (lookahead == 'm') ADVANCE(268);
      END_STATE();
    case 113:
      if (lookahead == 'a') ADVANCE(269);
      if (lookahead == 'p') ADVANCE(270);
      END_STATE();
    case 114:
      if (lookahead == 'l') ADVANCE(271);
      if (lookahead == 'o') ADVANCE(272);
      if (lookahead == 's') ADVANCE(273);
      if (lookahead == 't') ADVANCE(274);
      if (lookahead == 'v') ADVANCE(275);
      END_STATE();
    case 115:
      if (lookahead == 'a') ADVANCE(276);
      if (lookahead == 'u') ADVANCE(277);
      END_STATE();
    case 116:
      if (lookahead == 'e') ADVANCE(278);
      END_STATE();
    case 117:
      if (lookahead == 'y') ADVANCE(279);
      END_STATE();
    case 118:
      if (lookahead == 't') ADVANCE(280);
      END_STATE();
    case 119:
      if (lookahead == 'n') ADVANCE(281);
      END_STATE();
    case 120:
      if (lookahead == 'o') ADVANCE(282);
      if (lookahead == 'w') ADVANCE(283);
      END_STATE();
    case 121:
      if (lookahead == 'c') ADVANCE(284);
      if (lookahead == 't') ADVANCE(285);
      END_STATE();
    case 122:
      if (lookahead == 'd') ADVANCE(286);
      if (lookahead == 't') ADVANCE(287);
      END_STATE();
    case 123:
      if (lookahead == 'd') ADVANCE(288);
      if (lookahead == 'v') ADVANCE(289);
      END_STATE();
    case 124:
      if (lookahead == 't') ADVANCE(290);
      END_STATE();
    case 125:
      if (lookahead == 'm') ADVANCE(291);
      END_STATE();
    case 126:
      if (lookahead == 'w') ADVANCE(292);
      END_STATE();
    case 127:
      if (lookahead == 'l') ADVANCE(293);
      END_STATE();
    case 128:
      if (lookahead == 'e') ADVANCE(294);
      if (lookahead == 'i') ADVANCE(295);
      if (lookahead == 'p') ADVANCE(296);
      END_STATE();
    case 129:
      if (lookahead == 'l') ADVANCE(297);
      END_STATE();
    case 130:
      ACCEPT_TOKEN(anon_sym_of);
      END_STATE();
    case 131:
      if (lookahead == 'e') ADVANCE(298);
      END_STATE();
    case 132:
      if (lookahead == 'e') ADVANCE(299);
      END_STATE();
    case 133:
      if (lookahead == 'c') ADVANCE(300);
      if (lookahead == 'r') ADVANCE(301);
      if (lookahead == 's') ADVANCE(302);
      if (lookahead == 't') ADVANCE(303);
      END_STATE();
    case 134:
      if (lookahead == 'x') ADVANCE(304);
      END_STATE();
    case 135:
      if (lookahead == 'i') ADVANCE(305);
      END_STATE();
    case 136:
      if (lookahead == 'e') ADVANCE(306);
      if (lookahead == 'i') ADVANCE(307);
      if (lookahead == 'o') ADVANCE(308);
      END_STATE();
    case 137:
      if (lookahead == 'r') ADVANCE(309);
      END_STATE();
    case 138:
      if (lookahead == 'b') ADVANCE(310);
      END_STATE();
    case 139:
      if (lookahead == '2') ADVANCE(311);
      END_STATE();
    case 140:
      if (lookahead == 'a') ADVANCE(312);
      if (lookahead == 'f') ADVANCE(313);
      if (lookahead == 'g') ADVANCE(314);
      if (lookahead == 'i') ADVANCE(315);
      if (lookahead == 'q') ADVANCE(316);
      if (lookahead == 's') ADVANCE(317);
      if (lookahead == 't') ADVANCE(318);
      END_STATE();
    case 141:
      if (lookahead == '3') ADVANCE(319);
      if (lookahead == 'b') ADVANCE(320);
      END_STATE();
    case 142:
      if (lookahead == 'w') ADVANCE(321);
      END_STATE();
    case 143:
      if (lookahead == 'm') ADVANCE(322);
      END_STATE();
    case 144:
      if (lookahead == 'l') ADVANCE(323);
      if (lookahead == 't') ADVANCE(324);
      END_STATE();
    case 145:
      if (lookahead == 'a') ADVANCE(325);
      END_STATE();
    case 146:
      if (lookahead == 'g') ADVANCE(326);
      if (lookahead == 'z') ADVANCE(327);
      END_STATE();
    case 147:
      if (lookahead == 'o') ADVANCE(328);
      END_STATE();
    case 148:
      if (lookahead == 'o') ADVANCE(329);
      END_STATE();
    case 149:
      if (lookahead == 'a') ADVANCE(330);
      if (lookahead == 'd') ADVANCE(331);
      if (lookahead == 'o') ADVANCE(332);
      if (lookahead == 'r') ADVANCE(333);
      END_STATE();
    case 150:
      if (lookahead == 'b') ADVANCE(334);
      if (lookahead == 'p') ADVANCE(335);
      END_STATE();
    case 151:
      if (lookahead == 'i') ADVANCE(336);
      END_STATE();
    case 152:
      if (lookahead == 'r') ADVANCE(337);
      END_STATE();
    case 153:
      if (lookahead == 'u') ADVANCE(338);
      END_STATE();
    case 154:
      if (lookahead == 'c') ADVANCE(339);
      if (lookahead == 'm') ADVANCE(340);
      if (lookahead == 'x') ADVANCE(341);
      END_STATE();
    case 155:
      if (lookahead == 'i') ADVANCE(342);
      if (lookahead == 'r') ADVANCE(343);
      END_STATE();
    case 156:
      if (lookahead == 'a') ADVANCE(344);
      if (lookahead == 'i') ADVANCE(345);
      if (lookahead == 'u') ADVANCE(346);
      if (lookahead == 'y') ADVANCE(347);
      END_STATE();
    case 157:
      if (lookahead == 'p') ADVANCE(348);
      END_STATE();
    case 158:
      if (lookahead == '6') ADVANCE(349);
      END_STATE();
    case 159:
      if (lookahead == '2') ADVANCE(350);
      END_STATE();
    case 160:
      if (lookahead == '4') ADVANCE(351);
      END_STATE();
    case 161:
      ACCEPT_TOKEN(anon_sym_u8);
      END_STATE();
    case 162:
      if (lookahead == 'm') ADVANCE(352);
      END_STATE();
    case 163:
      if (lookahead == 'i') ADVANCE(353);
      if (lookahead == 'l') ADVANCE(354);
      if (lookahead == 'o') ADVANCE(355);
      if (lookahead == 's') ADVANCE(356);
      END_STATE();
    case 164:
      if (lookahead == 'a') ADVANCE(357);
      if (lookahead == 'e') ADVANCE(358);
      if (lookahead == 'i') ADVANCE(359);
      if (lookahead == 'u') ADVANCE(360);
      END_STATE();
    case 165:
      if (lookahead == 'e') ADVANCE(361);
      END_STATE();
    case 166:
      if (lookahead == 'r') ADVANCE(362);
      END_STATE();
    case 167:
      if (lookahead == 'c') ADVANCE(363);
      if (lookahead == 'r') ADVANCE(364);
      END_STATE();
    case 168:
      if (lookahead == 'r') ADVANCE(365);
      END_STATE();
    case 169:
      if (lookahead == 'i') ADVANCE(366);
      if (lookahead == 'l') ADVANCE(367);
      END_STATE();
    case 170:
      if (lookahead == 'h') ADVANCE(368);
      END_STATE();
    case 171:
      if (lookahead == 's') ADVANCE(369);
      END_STATE();
    case 172:
      if (lookahead == 'e') ADVANCE(370);
      if (lookahead == 'i') ADVANCE(371);
      END_STATE();
    case 173:
      if (lookahead == 't') ADVANCE(372);
      END_STATE();
    case 174:
      if (lookahead == 'r') ADVANCE(373);
      END_STATE();
    case 175:
      if (lookahead == 'i') ADVANCE(374);
      END_STATE();
    case 176:
      if (lookahead == 'e') ADVANCE(375);
      END_STATE();
    case 177:
      if (lookahead == 'e') ADVANCE(376);
      END_STATE();
    case 178:
      if (lookahead == 'n') ADVANCE(377);
      END_STATE();
    case 179:
      if (lookahead == 'f') ADVANCE(378);
      END_STATE();
    case 180:
      if (lookahead == 'e') ADVANCE(379);
      END_STATE();
    case 181:
      if (lookahead == 'p') ADVANCE(380);
      END_STATE();
    case 182:
      if (lookahead == 's') ADVANCE(381);
      END_STATE();
    case 183:
      if (lookahead == 't') ADVANCE(382);
      END_STATE();
    case 184:
      if (lookahead == 'a') ADVANCE(383);
      END_STATE();
    case 185:
      if (lookahead == 'm') ADVANCE(384);
      END_STATE();
    case 186:
      if (lookahead == 'l') ADVANCE(385);
      END_STATE();
    case 187:
      if (lookahead == 'u') ADVANCE(386);
      END_STATE();
    case 188:
      if (lookahead == 'e') ADVANCE(387);
      END_STATE();
    case 189:
      if (lookahead == 'L') ADVANCE(388);
      END_STATE();
    case 190:
      if (lookahead == 'p') ADVANCE(389);
      END_STATE();
    case 191:
      if (lookahead == 'e') ADVANCE(390);
      END_STATE();
    case 192:
      if (lookahead == 'n') ADVANCE(391);
      END_STATE();
    case 193:
      if (lookahead == 'u') ADVANCE(392);
      if (lookahead == 'y') ADVANCE(393);
      END_STATE();
    case 194:
      if (lookahead == 't') ADVANCE(394);
      END_STATE();
    case 195:
      if (lookahead == 'e') ADVANCE(395);
      END_STATE();
    case 196:
      if (lookahead == 't') ADVANCE(396);
      END_STATE();
    case 197:
      if (lookahead == 'd') ADVANCE(397);
      END_STATE();
    case 198:
      if (lookahead == 'p') ADVANCE(398);
      END_STATE();
    case 199:
      if (lookahead == 'f') ADVANCE(399);
      END_STATE();
    case 200:
      if (lookahead == 'u') ADVANCE(400);
      END_STATE();
    case 201:
      if (lookahead == 't') ADVANCE(401);
      END_STATE();
    case 202:
      if (lookahead == 'a') ADVANCE(402);
      END_STATE();
    case 203:
      if (lookahead == 't') ADVANCE(403);
      END_STATE();
    case 204:
      if (lookahead == 't') ADVANCE(404);
      END_STATE();
    case 205:
      if (lookahead == 'i') ADVANCE(405);
      END_STATE();
    case 206:
      if (lookahead == 'g') ADVANCE(406);
      END_STATE();
    case 207:
      if (lookahead == 'a') ADVANCE(407);
      END_STATE();
    case 208:
      ACCEPT_TOKEN(anon_sym_asm);
      if (lookahead == '_') ADVANCE(408);
      END_STATE();
    case 209:
      if (lookahead == 'n') ADVANCE(409);
      END_STATE();
    case 210:
      if (lookahead == 'm') ADVANCE(410);
      END_STATE();
    case 211:
      if (lookahead == 'r') ADVANCE(411);
      END_STATE();
    case 212:
      if (lookahead == 'o') ADVANCE(412);
      END_STATE();
    case 213:
      if (lookahead == 'i') ADVANCE(413);
      END_STATE();
    case 214:
      if (lookahead == 'o') ADVANCE(414);
      END_STATE();
    case 215:
      if (lookahead == '6') ADVANCE(415);
      END_STATE();
    case 216:
      if (lookahead == 'd') ADVANCE(416);
      END_STATE();
    case 217:
      if (lookahead == 'c') ADVANCE(417);
      END_STATE();
    case 218:
      if (lookahead == 'l') ADVANCE(418);
      END_STATE();
    case 219:
      if (lookahead == 'a') ADVANCE(419);
      END_STATE();
    case 220:
      if (lookahead == 'e') ADVANCE(420);
      if (lookahead == 't') ADVANCE(421);
      END_STATE();
    case 221:
      if (lookahead == 'c') ADVANCE(422);
      END_STATE();
    case 222:
      if (lookahead == 'f') ADVANCE(423);
      END_STATE();
    case 223:
      if (lookahead == 't') ADVANCE(424);
      END_STATE();
    case 224:
      if (lookahead == 'r') ADVANCE(425);
      END_STATE();
    case 225:
      if (lookahead == 's') ADVANCE(426);
      END_STATE();
    case 226:
      if (lookahead == 'a') ADVANCE(427);
      if (lookahead == 'r') ADVANCE(428);
      if (lookahead == 'y') ADVANCE(429);
      END_STATE();
    case 227:
      if (lookahead == 'e') ADVANCE(430);
      END_STATE();
    case 228:
      if (lookahead == 'u') ADVANCE(431);
      END_STATE();
    case 229:
      if (lookahead == 'm') ADVANCE(432);
      if (lookahead == 'p') ADVANCE(433);
      END_STATE();
    case 230:
      if (lookahead == 'c') ADVANCE(434);
      if (lookahead == 's') ADVANCE(435);
      if (lookahead == 't') ADVANCE(436);
      END_STATE();
    case 231:
      if (lookahead == 't') ADVANCE(437);
      END_STATE();
    case 232:
      if (lookahead == 'u') ADVANCE(438);
      END_STATE();
    case 233:
      if (lookahead == 'l') ADVANCE(439);
      END_STATE();
    case 234:
      if (lookahead == 'a') ADVANCE(440);
      END_STATE();
    case 235:
      if (lookahead == 'e') ADVANCE(441);
      END_STATE();
    case 236:
      if (lookahead == 'o') ADVANCE(442);
      END_STATE();
    case 237:
      if (lookahead == 'c') ADVANCE(443);
      END_STATE();
    case 238:
      if (lookahead == 'r') ADVANCE(444);
      END_STATE();
    case 239:
      if (lookahead == 'a') ADVANCE(445);
      END_STATE();
    case 240:
      if (lookahead == 'e') ADVANCE(446);
      END_STATE();
    case 241:
      if (lookahead == 'b') ADVANCE(447);
      END_STATE();
    case 242:
      if (lookahead == 'm') ADVANCE(448);
      END_STATE();
    case 243:
      if (lookahead == 'l') ADVANCE(449);
      if (lookahead == 'o') ADVANCE(450);
      END_STATE();
    case 244:
      if (lookahead == 'e') ADVANCE(451);
      END_STATE();
    case 245:
      ACCEPT_TOKEN(sym_float16);
      END_STATE();
    case 246:
      ACCEPT_TOKEN(sym_float32);
      END_STATE();
    case 247:
      ACCEPT_TOKEN(anon_sym_f64);
      END_STATE();
    case 248:
      if (lookahead == 'l') ADVANCE(452);
      if (lookahead == 's') ADVANCE(453);
      END_STATE();
    case 249:
      if (lookahead == 't') ADVANCE(454);
      END_STATE();
    case 250:
      if (lookahead == 'a') ADVANCE(455);
      END_STATE();
    case 251:
      if (lookahead == 'e') ADVANCE(456);
      END_STATE();
    case 252:
      if (lookahead == 't') ADVANCE(457);
      END_STATE();
    case 253:
      ACCEPT_TOKEN(sym_for);
      END_STATE();
    case 254:
      if (lookahead == 'e') ADVANCE(458);
      END_STATE();
    case 255:
      if (lookahead == 'm') ADVANCE(459);
      END_STATE();
    case 256:
      if (lookahead == 'c') ADVANCE(460);
      END_STATE();
    case 257:
      if (lookahead == 'c') ADVANCE(461);
      END_STATE();
    case 258:
      if (lookahead == 'r') ADVANCE(462);
      END_STATE();
    case 259:
      ACCEPT_TOKEN(anon_sym_get);
      END_STATE();
    case 260:
      if (lookahead == 'o') ADVANCE(463);
      END_STATE();
    case 261:
      if (lookahead == 'u') ADVANCE(464);
      END_STATE();
    case 262:
      if (lookahead == 'd') ADVANCE(465);
      END_STATE();
    case 263:
      if (lookahead == 'h') ADVANCE(466);
      END_STATE();
    case 264:
      if (lookahead == 'c') ADVANCE(467);
      END_STATE();
    case 265:
      ACCEPT_TOKEN(anon_sym_i16);
      END_STATE();
    case 266:
      ACCEPT_TOKEN(sym_int32);
      END_STATE();
    case 267:
      ACCEPT_TOKEN(anon_sym_i64);
      END_STATE();
    case 268:
      if (lookahead == 'a') ADVANCE(468);
      END_STATE();
    case 269:
      if (lookahead == 'g') ADVANCE(469);
      END_STATE();
    case 270:
      if (lookahead == 'l') ADVANCE(470);
      if (lookahead == 'o') ADVANCE(471);
      END_STATE();
    case 271:
      if (lookahead == 'i') ADVANCE(472);
      END_STATE();
    case 272:
      if (lookahead == 'u') ADVANCE(473);
      END_STATE();
    case 273:
      if (lookahead == 't') ADVANCE(474);
      END_STATE();
    case 274:
      if (lookahead == 'e') ADVANCE(475);
      END_STATE();
    case 275:
      if (lookahead == 'a') ADVANCE(476);
      END_STATE();
    case 276:
      if (lookahead == 'm') ADVANCE(477);
      END_STATE();
    case 277:
      if (lookahead == 'b') ADVANCE(478);
      END_STATE();
    case 278:
      if (lookahead == 'x') ADVANCE(479);
      END_STATE();
    case 279:
      if (lookahead == 'o') ADVANCE(480);
      END_STATE();
    case 280:
      ACCEPT_TOKEN(sym_let);
      END_STATE();
    case 281:
      if (lookahead == 'e') ADVANCE(481);
      END_STATE();
    case 282:
      if (lookahead == 'p') ADVANCE(482);
      END_STATE();
    case 283:
      if (lookahead == 'p') ADVANCE(483);
      END_STATE();
    case 284:
      if (lookahead == 'r') ADVANCE(484);
      END_STATE();
    case 285:
      ACCEPT_TOKEN(anon_sym_mat);
      if (lookahead == '2') ADVANCE(485);
      if (lookahead == '3') ADVANCE(486);
      if (lookahead == '4') ADVANCE(487);
      if (lookahead == 'c') ADVANCE(488);
      if (lookahead == 'r') ADVANCE(489);
      END_STATE();
    case 286:
      if (lookahead == 'i') ADVANCE(490);
      END_STATE();
    case 287:
      if (lookahead == 'a') ADVANCE(491);
      END_STATE();
    case 288:
      ACCEPT_TOKEN(anon_sym_mod);
      if (lookahead == 'u') ADVANCE(492);
      END_STATE();
    case 289:
      if (lookahead == 'e') ADVANCE(493);
      END_STATE();
    case 290:
      ACCEPT_TOKEN(anon_sym_mut);
      if (lookahead == 'a') ADVANCE(494);
      END_STATE();
    case 291:
      if (lookahead == 'e') ADVANCE(495);
      END_STATE();
    case 292:
      ACCEPT_TOKEN(anon_sym_new);
      END_STATE();
    case 293:
      ACCEPT_TOKEN(anon_sym_nil);
      END_STATE();
    case 294:
      if (lookahead == 'x') ADVANCE(496);
      END_STATE();
    case 295:
      if (lookahead == 'n') ADVANCE(497);
      END_STATE();
    case 296:
      if (lookahead == 'e') ADVANCE(498);
      END_STATE();
    case 297:
      if (lookahead == 'l') ADVANCE(499);
      END_STATE();
    case 298:
      if (lookahead == 'r') ADVANCE(500);
      END_STATE();
    case 299:
      if (lookahead == 'r') ADVANCE(501);
      END_STATE();
    case 300:
      if (lookahead == 'k') ADVANCE(502);
      END_STATE();
    case 301:
      if (lookahead == 't') ADVANCE(503);
      END_STATE();
    case 302:
      if (lookahead == 's') ADVANCE(504);
      END_STATE();
    case 303:
      if (lookahead == 'c') ADVANCE(505);
      END_STATE();
    case 304:
      if (lookahead == 'e') ADVANCE(506);
      END_STATE();
    case 305:
      if (lookahead == 'n') ADVANCE(507);
      END_STATE();
    case 306:
      if (lookahead == 'c') ADVANCE(508);
      if (lookahead == 'm') ADVANCE(509);
      END_STATE();
    case 307:
      if (lookahead == 'v') ADVANCE(510);
      END_STATE();
    case 308:
      if (lookahead == 't') ADVANCE(511);
      END_STATE();
    case 309:
      ACCEPT_TOKEN(sym_pointer);
      END_STATE();
    case 310:
      ACCEPT_TOKEN(anon_sym_pub);
      if (lookahead == 'l') ADVANCE(512);
      END_STATE();
    case 311:
      if (lookahead == 'f') ADVANCE(513);
      if (lookahead == 's') ADVANCE(514);
      if (lookahead == 'u') ADVANCE(515);
      END_STATE();
    case 312:
      if (lookahead == 'd') ADVANCE(516);
      END_STATE();
    case 313:
      ACCEPT_TOKEN(anon_sym_ref);
      END_STATE();
    case 314:
      if (lookahead == 'a') ADVANCE(517);
      if (lookahead == 'i') ADVANCE(518);
      END_STATE();
    case 315:
      if (lookahead == 'n') ADVANCE(519);
      END_STATE();
    case 316:
      if (lookahead == 'u') ADVANCE(520);
      END_STATE();
    case 317:
      if (lookahead == 'o') ADVANCE(521);
      if (lookahead == 't') ADVANCE(522);
      END_STATE();
    case 318:
      if (lookahead == 'u') ADVANCE(523);
      END_STATE();
    case 319:
      if (lookahead == '2') ADVANCE(524);
      END_STATE();
    case 320:
      if (lookahead == 'a') ADVANCE(525);
      END_STATE();
    case 321:
      if (lookahead == '_') ADVANCE(526);
      END_STATE();
    case 322:
      if (lookahead == 'p') ADVANCE(527);
      END_STATE();
    case 323:
      if (lookahead == 'f') ADVANCE(528);
      END_STATE();
    case 324:
      ACCEPT_TOKEN(anon_sym_set);
      END_STATE();
    case 325:
      if (lookahead == 'r') ADVANCE(529);
      END_STATE();
    case 326:
      if (lookahead == 'n') ADVANCE(530);
      END_STATE();
    case 327:
      if (lookahead == 'e') ADVANCE(531);
      END_STATE();
    case 328:
      if (lookahead == 'o') ADVANCE(532);
      END_STATE();
    case 329:
      if (lookahead == 'r') ADVANCE(533);
      END_STATE();
    case 330:
      if (lookahead == 't') ADVANCE(534);
      END_STATE();
    case 331:
      ACCEPT_TOKEN(anon_sym_std);
      END_STATE();
    case 332:
      if (lookahead == 'r') ADVANCE(535);
      END_STATE();
    case 333:
      if (lookahead == 'i') ADVANCE(536);
      if (lookahead == 'u') ADVANCE(537);
      END_STATE();
    case 334:
      if (lookahead == 'p') ADVANCE(538);
      if (lookahead == 'r') ADVANCE(539);
      END_STATE();
    case 335:
      if (lookahead == 'e') ADVANCE(540);
      END_STATE();
    case 336:
      if (lookahead == 't') ADVANCE(541);
      END_STATE();
    case 337:
      if (lookahead == 'g') ADVANCE(542);
      END_STATE();
    case 338:
      if (lookahead == 'f') ADVANCE(543);
      END_STATE();
    case 339:
      if (lookahead == 'h') ADVANCE(544);
      END_STATE();
    case 340:
      if (lookahead == 'p') ADVANCE(545);
      END_STATE();
    case 341:
      if (lookahead == 't') ADVANCE(546);
      END_STATE();
    case 342:
      if (lookahead == 's') ADVANCE(547);
      END_STATE();
    case 343:
      if (lookahead == 'e') ADVANCE(548);
      if (lookahead == 'o') ADVANCE(549);
      END_STATE();
    case 344:
      if (lookahead == 'i') ADVANCE(550);
      END_STATE();
    case 345:
      if (lookahead == 'a') ADVANCE(551);
      END_STATE();
    case 346:
      if (lookahead == 'e') ADVANCE(552);
      END_STATE();
    case 347:
      ACCEPT_TOKEN(anon_sym_try);
      END_STATE();
    case 348:
      if (lookahead == 'e') ADVANCE(553);
      END_STATE();
    case 349:
      ACCEPT_TOKEN(anon_sym_u16);
      END_STATE();
    case 350:
      ACCEPT_TOKEN(sym_uint32);
      END_STATE();
    case 351:
      ACCEPT_TOKEN(anon_sym_u64);
      END_STATE();
    case 352:
      if (lookahead == 'a') ADVANCE(554);
      END_STATE();
    case 353:
      if (lookahead == 'f') ADVANCE(555);
      if (lookahead == 'o') ADVANCE(556);
      END_STATE();
    case 354:
      if (lookahead == 'e') ADVANCE(557);
      END_STATE();
    case 355:
      if (lookahead == 'r') ADVANCE(558);
      END_STATE();
    case 356:
      if (lookahead == 'a') ADVANCE(559);
      if (lookahead == 'i') ADVANCE(560);
      END_STATE();
    case 357:
      if (lookahead == 'm') ADVANCE(561);
      END_STATE();
    case 358:
      ACCEPT_TOKEN(anon_sym_use);
      END_STATE();
    case 359:
      if (lookahead == 'n') ADVANCE(562);
      END_STATE();
    case 360:
      if (lookahead == 'b') ADVANCE(563);
      END_STATE();
    case 361:
      if (lookahead == 'x') ADVANCE(564);
      END_STATE();
    case 362:
      ACCEPT_TOKEN(sym_var);
      if (lookahead == 'y') ADVANCE(565);
      END_STATE();
    case 363:
      ACCEPT_TOKEN(anon_sym_vec);
      if (lookahead == '2') ADVANCE(566);
      if (lookahead == '3') ADVANCE(567);
      if (lookahead == '4') ADVANCE(568);
      if (lookahead == 't') ADVANCE(569);
      END_STATE();
    case 364:
      if (lookahead == 't') ADVANCE(570);
      END_STATE();
    case 365:
      if (lookahead == 't') ADVANCE(571);
      END_STATE();
    case 366:
      if (lookahead == 'd') ADVANCE(572);
      END_STATE();
    case 367:
      if (lookahead == 'a') ADVANCE(573);
      END_STATE();
    case 368:
      if (lookahead == 'a') ADVANCE(574);
      END_STATE();
    case 369:
      if (lookahead == 'l') ADVANCE(575);
      END_STATE();
    case 370:
      if (lookahead == 'r') ADVANCE(576);
      END_STATE();
    case 371:
      if (lookahead == 'l') ADVANCE(577);
      END_STATE();
    case 372:
      if (lookahead == 'h') ADVANCE(578);
      END_STATE();
    case 373:
      if (lookahead == 'k') ADVANCE(579);
      END_STATE();
    case 374:
      if (lookahead == 't') ADVANCE(580);
      END_STATE();
    case 375:
      if (lookahead == 'l') ADVANCE(581);
      END_STATE();
    case 376:
      if (lookahead == 'n') ADVANCE(582);
      END_STATE();
    case 377:
      if (lookahead == 'd') ADVANCE(583);
      END_STATE();
    case 378:
      if (lookahead == 'e') ADVANCE(584);
      END_STATE();
    case 379:
      if (lookahead == 'A') ADVANCE(585);
      END_STATE();
    case 380:
      if (lookahead == 'i') ADVANCE(586);
      if (lookahead == 'u') ADVANCE(587);
      END_STATE();
    case 381:
      if (lookahead == 'u') ADVANCE(588);
      END_STATE();
    case 382:
      if (lookahead == 'h') ADVANCE(589);
      END_STATE();
    case 383:
      if (lookahead == 'i') ADVANCE(590);
      END_STATE();
    case 384:
      if (lookahead == 'e') ADVANCE(591);
      END_STATE();
    case 385:
      if (lookahead == 's') ADVANCE(592);
      END_STATE();
    case 386:
      if (lookahead == 't') ADVANCE(593);
      END_STATE();
    case 387:
      if (lookahead == 'S') ADVANCE(594);
      END_STATE();
    case 388:
      ACCEPT_TOKEN(anon_sym_NULL);
      END_STATE();
    case 389:
      if (lookahead == 'u') ADVANCE(595);
      END_STATE();
    case 390:
      if (lookahead == 'l') ADVANCE(596);
      END_STATE();
    case 391:
      if (lookahead == 't') ADVANCE(597);
      END_STATE();
    case 392:
      if (lookahead == 'f') ADVANCE(598);
      END_STATE();
    case 393:
      if (lookahead == 't') ADVANCE(599);
      END_STATE();
    case 394:
      if (lookahead == 'r') ADVANCE(600);
      END_STATE();
    case 395:
      if (lookahead == 'x') ADVANCE(601);
      END_STATE();
    case 396:
      if (lookahead == 'e') ADVANCE(602);
      END_STATE();
    case 397:
      if (lookahead == 'e') ADVANCE(603);
      END_STATE();
    case 398:
      if (lookahead == 'l') ADVANCE(604);
      END_STATE();
    case 399:
      ACCEPT_TOKEN(anon_sym_Self);
      END_STATE();
    case 400:
      if (lookahead == 'c') ADVANCE(605);
      END_STATE();
    case 401:
      if (lookahead == 'u') ADVANCE(606);
      END_STATE();
    case 402:
      if (lookahead == 'n') ADVANCE(607);
      END_STATE();
    case 403:
      if (lookahead == 'e') ADVANCE(608);
      END_STATE();
    case 404:
      if (lookahead == 'r') ADVANCE(609);
      END_STATE();
    case 405:
      if (lookahead == 'v') ADVANCE(610);
      END_STATE();
    case 406:
      if (lookahead == 'n') ADVANCE(611);
      END_STATE();
    case 407:
      if (lookahead == 'y') ADVANCE(612);
      END_STATE();
    case 408:
      if (lookahead == 'f') ADVANCE(613);
      END_STATE();
    case 409:
      if (lookahead == 'c') ADVANCE(614);
      END_STATE();
    case 410:
      if (lookahead == 'i') ADVANCE(615);
      END_STATE();
    case 411:
      if (lookahead == 'i') ADVANCE(616);
      END_STATE();
    case 412:
      ACCEPT_TOKEN(anon_sym_auto);
      END_STATE();
    case 413:
      if (lookahead == 't') ADVANCE(617);
      END_STATE();
    case 414:
      if (lookahead == 'm') ADVANCE(618);
      END_STATE();
    case 415:
      ACCEPT_TOKEN(anon_sym_bf16);
      END_STATE();
    case 416:
      if (lookahead == 'i') ADVANCE(619);
      END_STATE();
    case 417:
      if (lookahead == 'a') ADVANCE(620);
      END_STATE();
    case 418:
      ACCEPT_TOKEN(sym_bool);
      END_STATE();
    case 419:
      if (lookahead == 'k') ADVANCE(621);
      END_STATE();
    case 420:
      ACCEPT_TOKEN(sym_case);
      END_STATE();
    case 421:
      ACCEPT_TOKEN(anon_sym_cast);
      END_STATE();
    case 422:
      if (lookahead == 'h') ADVANCE(622);
      END_STATE();
    case 423:
      if (lookahead == 'f') ADVANCE(623);
      END_STATE();
    case 424:
      if (lookahead == 'r') ADVANCE(624);
      END_STATE();
    case 425:
      ACCEPT_TOKEN(anon_sym_char);
      END_STATE();
    case 426:
      if (lookahead == 's') ADVANCE(625);
      END_STATE();
    case 427:
      if (lookahead == 'w') ADVANCE(626);
      END_STATE();
    case 428:
      if (lookahead == 'e') ADVANCE(627);
      END_STATE();
    case 429:
      if (lookahead == 'i') ADVANCE(628);
      END_STATE();
    case 430:
      if (lookahead == 'r') ADVANCE(629);
      END_STATE();
    case 431:
      if (lookahead == 'm') ADVANCE(630);
      END_STATE();
    case 432:
      if (lookahead == 'o') ADVANCE(631);
      END_STATE();
    case 433:
      if (lookahead == 'i') ADVANCE(632);
      END_STATE();
    case 434:
      if (lookahead == 'e') ADVANCE(633);
      END_STATE();
    case 435:
      if (lookahead == 't') ADVANCE(634);
      END_STATE();
    case 436:
      if (lookahead == 'i') ADVANCE(635);
      END_STATE();
    case 437:
      if (lookahead == 'e') ADVANCE(636);
      END_STATE();
    case 438:
      if (lookahead == 'g') ADVANCE(637);
      END_STATE();
    case 439:
      if (lookahead == 't') ADVANCE(638);
      END_STATE();
    case 440:
      if (lookahead == 'u') ADVANCE(639);
      END_STATE();
    case 441:
      if (lookahead == 't') ADVANCE(640);
      END_STATE();
    case 442:
      if (lookahead == 't') ADVANCE(641);
      END_STATE();
    case 443:
      if (lookahead == 'a') ADVANCE(642);
      END_STATE();
    case 444:
      if (lookahead == 'd') ADVANCE(643);
      END_STATE();
    case 445:
      if (lookahead == 'm') ADVANCE(644);
      END_STATE();
    case 446:
      ACCEPT_TOKEN(sym_else);
      END_STATE();
    case 447:
      if (lookahead == 'l') ADVANCE(645);
      END_STATE();
    case 448:
      ACCEPT_TOKEN(anon_sym_enum);
      END_STATE();
    case 449:
      if (lookahead == 'i') ADVANCE(646);
      END_STATE();
    case 450:
      if (lookahead == 'r') ADVANCE(647);
      END_STATE();
    case 451:
      if (lookahead == 'n') ADVANCE(648);
      if (lookahead == 'r') ADVANCE(649);
      END_STATE();
    case 452:
      if (lookahead == 't') ADVANCE(650);
      END_STATE();
    case 453:
      if (lookahead == 'e') ADVANCE(651);
      END_STATE();
    case 454:
      if (lookahead == 'e') ADVANCE(652);
      END_STATE();
    case 455:
      if (lookahead == 'l') ADVANCE(653);
      END_STATE();
    case 456:
      if (lookahead == 'd') ADVANCE(654);
      END_STATE();
    case 457:
      ACCEPT_TOKEN(anon_sym_flat);
      END_STATE();
    case 458:
      if (lookahead == 'n') ADVANCE(655);
      END_STATE();
    case 459:
      ACCEPT_TOKEN(anon_sym_from);
      END_STATE();
    case 460:
      if (lookahead == 't') ADVANCE(656);
      END_STATE();
    case 461:
      if (lookahead == '2') ADVANCE(657);
      if (lookahead == '3') ADVANCE(658);
      if (lookahead == '4') ADVANCE(659);
      END_STATE();
    case 462:
      if (lookahead == 'o') ADVANCE(660);
      END_STATE();
    case 463:
      ACCEPT_TOKEN(anon_sym_goto);
      END_STATE();
    case 464:
      if (lookahead == 'p') ADVANCE(661);
      END_STATE();
    case 465:
      if (lookahead == 'l') ADVANCE(662);
      END_STATE();
    case 466:
      if (lookahead == 'p') ADVANCE(663);
      END_STATE();
    case 467:
      if (lookahead == '2') ADVANCE(664);
      if (lookahead == '3') ADVANCE(665);
      if (lookahead == '4') ADVANCE(666);
      END_STATE();
    case 468:
      if (lookahead == 'g') ADVANCE(667);
      END_STATE();
    case 469:
      if (lookahead == 'e') ADVANCE(668);
      END_STATE();
    case 470:
      ACCEPT_TOKEN(anon_sym_impl);
      if (lookahead == 'e') ADVANCE(669);
      END_STATE();
    case 471:
      if (lookahead == 'r') ADVANCE(670);
      END_STATE();
    case 472:
      if (lookahead == 'n') ADVANCE(671);
      END_STATE();
    case 473:
      if (lookahead == 't') ADVANCE(672);
      END_STATE();
    case 474:
      if (lookahead == 'a') ADVANCE(673);
      END_STATE();
    case 475:
      if (lookahead == 'r') ADVANCE(674);
      END_STATE();
    case 476:
      if (lookahead == 'r') ADVANCE(675);
      END_STATE();
    case 477:
      if (lookahead == 'p') ADVANCE(676);
      END_STATE();
    case 478:
      if (lookahead == 'p') ADVANCE(677);
      END_STATE();
    case 479:
      if (lookahead == 't') ADVANCE(678);
      END_STATE();
    case 480:
      if (lookahead == 'u') ADVANCE(679);
      END_STATE();
    case 481:
      ACCEPT_TOKEN(anon_sym_line);
      if (lookahead == 'a') ADVANCE(680);
      END_STATE();
    case 482:
      ACCEPT_TOKEN(sym_loop);
      END_STATE();
    case 483:
      ACCEPT_TOKEN(anon_sym_lowp);
      END_STATE();
    case 484:
      if (lookahead == 'o') ADVANCE(681);
      END_STATE();
    case 485:
      if (lookahead == 'x') ADVANCE(682);
      END_STATE();
    case 486:
      if (lookahead == 'x') ADVANCE(683);
      END_STATE();
    case 487:
      if (lookahead == 'x') ADVANCE(684);
      END_STATE();
    case 488:
      if (lookahead == 'h') ADVANCE(685);
      END_STATE();
    case 489:
      if (lookahead == 'i') ADVANCE(686);
      END_STATE();
    case 490:
      if (lookahead == 'u') ADVANCE(687);
      END_STATE();
    case 491:
      ACCEPT_TOKEN(anon_sym_meta);
      END_STATE();
    case 492:
      if (lookahead == 'l') ADVANCE(688);
      END_STATE();
    case 493:
      ACCEPT_TOKEN(anon_sym_move);
      END_STATE();
    case 494:
      if (lookahead == 'b') ADVANCE(689);
      END_STATE();
    case 495:
      if (lookahead == 's') ADVANCE(690);
      END_STATE();
    case 496:
      if (lookahead == 'c') ADVANCE(691);
      END_STATE();
    case 497:
      if (lookahead == 'l') ADVANCE(692);
      if (lookahead == 't') ADVANCE(693);
      END_STATE();
    case 498:
      if (lookahead == 'r') ADVANCE(694);
      END_STATE();
    case 499:
      ACCEPT_TOKEN(anon_sym_null);
      if (lookahead == 'p') ADVANCE(695);
      END_STATE();
    case 500:
      if (lookahead == 'a') ADVANCE(696);
      END_STATE();
    case 501:
      if (lookahead == 'r') ADVANCE(697);
      END_STATE();
    case 502:
      if (lookahead == 'a') ADVANCE(698);
      if (lookahead == 'o') ADVANCE(699);
      END_STATE();
    case 503:
      if (lookahead == 'i') ADVANCE(700);
      END_STATE();
    case 504:
      ACCEPT_TOKEN(anon_sym_pass);
      END_STATE();
    case 505:
      if (lookahead == 'h') ADVANCE(701);
      END_STATE();
    case 506:
      if (lookahead == 'l') ADVANCE(702);
      END_STATE();
    case 507:
      if (lookahead == 't') ADVANCE(703);
      END_STATE();
    case 508:
      if (lookahead == 'i') ADVANCE(704);
      END_STATE();
    case 509:
      if (lookahead == 'e') ADVANCE(705);
      END_STATE();
    case 510:
      ACCEPT_TOKEN(anon_sym_priv);
      if (lookahead == 'a') ADVANCE(706);
      END_STATE();
    case 511:
      if (lookahead == 'e') ADVANCE(707);
      END_STATE();
    case 512:
      if (lookahead == 'i') ADVANCE(708);
      END_STATE();
    case 513:
      if (lookahead == 'l') ADVANCE(709);
      END_STATE();
    case 514:
      if (lookahead == 'i') ADVANCE(710);
      END_STATE();
    case 515:
      if (lookahead == 'i') ADVANCE(711);
      END_STATE();
    case 516:
      ACCEPT_TOKEN(anon_sym_read);
      if (lookahead == '_') ADVANCE(712);
      if (lookahead == 'o') ADVANCE(713);
      END_STATE();
    case 517:
      if (lookahead == 'r') ADVANCE(714);
      END_STATE();
    case 518:
      if (lookahead == 's') ADVANCE(715);
      END_STATE();
    case 519:
      if (lookahead == 't') ADVANCE(716);
      END_STATE();
    case 520:
      if (lookahead == 'i') ADVANCE(717);
      END_STATE();
    case 521:
      if (lookahead == 'u') ADVANCE(718);
      END_STATE();
    case 522:
      if (lookahead == 'r') ADVANCE(719);
      END_STATE();
    case 523:
      if (lookahead == 'r') ADVANCE(720);
      END_STATE();
    case 524:
      if (lookahead == 'f') ADVANCE(721);
      if (lookahead == 's') ADVANCE(722);
      if (lookahead == 'u') ADVANCE(723);
      END_STATE();
    case 525:
      if (lookahead == '1') ADVANCE(724);
      if (lookahead == '3') ADVANCE(725);
      if (lookahead == '8') ADVANCE(726);
      END_STATE();
    case 526:
      if (lookahead == 'm') ADVANCE(727);
      END_STATE();
    case 527:
      if (lookahead == 'e') ADVANCE(728);
      if (lookahead == 'l') ADVANCE(729);
      END_STATE();
    case 528:
      ACCEPT_TOKEN(anon_sym_self);
      END_STATE();
    case 529:
      if (lookahead == 'e') ADVANCE(730);
      END_STATE();
    case 530:
      if (lookahead == 'e') ADVANCE(731);
      END_STATE();
    case 531:
      if (lookahead == 'o') ADVANCE(732);
      END_STATE();
    case 532:
      if (lookahead == 't') ADVANCE(733);
      END_STATE();
    case 533:
      if (lookahead == 'm') ADVANCE(734);
      END_STATE();
    case 534:
      if (lookahead == 'e') ADVANCE(735);
      if (lookahead == 'i') ADVANCE(736);
      END_STATE();
    case 535:
      if (lookahead == 'a') ADVANCE(737);
      END_STATE();
    case 536:
      if (lookahead == 'n') ADVANCE(738);
      END_STATE();
    case 537:
      if (lookahead == 'c') ADVANCE(739);
      END_STATE();
    case 538:
      if (lookahead == 'a') ADVANCE(740);
      END_STATE();
    case 539:
      if (lookahead == 'o') ADVANCE(741);
      END_STATE();
    case 540:
      if (lookahead == 'r') ADVANCE(742);
      END_STATE();
    case 541:
      if (lookahead == 'c') ADVANCE(743);
      END_STATE();
    case 542:
      if (lookahead == 'e') ADVANCE(744);
      END_STATE();
    case 543:
      if (lookahead == 'f') ADVANCE(745);
      END_STATE();
    case 544:
      if (lookahead == 'n') ADVANCE(746);
      END_STATE();
    case 545:
      if (lookahead == 'l') ADVANCE(747);
      END_STATE();
    case 546:
      if (lookahead == 'u') ADVANCE(748);
      END_STATE();
    case 547:
      ACCEPT_TOKEN(anon_sym_this);
      END_STATE();
    case 548:
      if (lookahead == 'a') ADVANCE(749);
      END_STATE();
    case 549:
      if (lookahead == 'w') ADVANCE(750);
      END_STATE();
    case 550:
      if (lookahead == 't') ADVANCE(751);
      END_STATE();
    case 551:
      if (lookahead == 'n') ADVANCE(752);
      END_STATE();
    case 552:
      ACCEPT_TOKEN(sym_true);
      END_STATE();
    case 553:
      ACCEPT_TOKEN(sym_type);
      if (lookahead == 'd') ADVANCE(753);
      if (lookahead == 'i') ADVANCE(754);
      if (lookahead == 'n') ADVANCE(755);
      if (lookahead == 'o') ADVANCE(756);
      END_STATE();
    case 554:
      if (lookahead == 'g') ADVANCE(757);
      END_STATE();
    case 555:
      if (lookahead == 'o') ADVANCE(758);
      END_STATE();
    case 556:
      if (lookahead == 'n') ADVANCE(759);
      END_STATE();
    case 557:
      if (lookahead == 's') ADVANCE(760);
      END_STATE();
    case 558:
      if (lookahead == 'm') ADVANCE(761);
      END_STATE();
    case 559:
      if (lookahead == 'f') ADVANCE(762);
      END_STATE();
    case 560:
      if (lookahead == 'g') ADVANCE(763);
      if (lookahead == 'z') ADVANCE(764);
      END_STATE();
    case 561:
      if (lookahead == 'p') ADVANCE(765);
      END_STATE();
    case 562:
      if (lookahead == 'g') ADVANCE(766);
      END_STATE();
    case 563:
      if (lookahead == 'p') ADVANCE(767);
      END_STATE();
    case 564:
      if (lookahead == 't') ADVANCE(768);
      END_STATE();
    case 565:
      if (lookahead == 'i') ADVANCE(769);
      END_STATE();
    case 566:
      ACCEPT_TOKEN(sym_vec2);
      END_STATE();
    case 567:
      ACCEPT_TOKEN(sym_vec3);
      END_STATE();
    case 568:
      ACCEPT_TOKEN(sym_vec4);
      END_STATE();
    case 569:
      if (lookahead == 'o') ADVANCE(770);
      END_STATE();
    case 570:
      if (lookahead == 'e') ADVANCE(771);
      END_STATE();
    case 571:
      if (lookahead == 'u') ADVANCE(772);
      END_STATE();
    case 572:
      ACCEPT_TOKEN(anon_sym_void);
      END_STATE();
    case 573:
      if (lookahead == 't') ADVANCE(773);
      END_STATE();
    case 574:
      if (lookahead == 'r') ADVANCE(774);
      END_STATE();
    case 575:
      ACCEPT_TOKEN(anon_sym_wgsl);
      END_STATE();
    case 576:
      if (lookahead == 'e') ADVANCE(775);
      END_STATE();
    case 577:
      if (lookahead == 'e') ADVANCE(776);
      END_STATE();
    case 578:
      ACCEPT_TOKEN(anon_sym_with);
      END_STATE();
    case 579:
      if (lookahead == 'g') ADVANCE(777);
      END_STATE();
    case 580:
      if (lookahead == 'e') ADVANCE(778);
      END_STATE();
    case 581:
      if (lookahead == 'd') ADVANCE(779);
      END_STATE();
    case 582:
      if (lookahead == 'd') ADVANCE(780);
      END_STATE();
    case 583:
      if (lookahead == 'S') ADVANCE(781);
      END_STATE();
    case 584:
      if (lookahead == 'r') ADVANCE(782);
      END_STATE();
    case 585:
      if (lookahead == 'd') ADVANCE(783);
      END_STATE();
    case 586:
      if (lookahead == 'l') ADVANCE(784);
      END_STATE();
    case 587:
      if (lookahead == 't') ADVANCE(785);
      END_STATE();
    case 588:
      if (lookahead == 'm') ADVANCE(786);
      END_STATE();
    case 589:
      if (lookahead == 'S') ADVANCE(787);
      END_STATE();
    case 590:
      if (lookahead == 'n') ADVANCE(788);
      END_STATE();
    case 591:
      if (lookahead == 't') ADVANCE(789);
      END_STATE();
    case 592:
      if (lookahead == 'h') ADVANCE(790);
      END_STATE();
    case 593:
      if (lookahead == 'P') ADVANCE(791);
      END_STATE();
    case 594:
      if (lookahead == 't') ADVANCE(792);
      END_STATE();
    case 595:
      if (lookahead == 't') ADVANCE(793);
      END_STATE();
    case 596:
      if (lookahead == 'S') ADVANCE(794);
      END_STATE();
    case 597:
      if (lookahead == 'S') ADVANCE(795);
      END_STATE();
    case 598:
      if (lookahead == 'f') ADVANCE(796);
      END_STATE();
    case 599:
      if (lookahead == 'e') ADVANCE(797);
      END_STATE();
    case 600:
      if (lookahead == 'u') ADVANCE(798);
      END_STATE();
    case 601:
      if (lookahead == 't') ADVANCE(799);
      END_STATE();
    case 602:
      if (lookahead == 'r') ADVANCE(800);
      END_STATE();
    case 603:
      if (lookahead == 'r') ADVANCE(801);
      END_STATE();
    case 604:
      if (lookahead == 'e') ADVANCE(802);
      END_STATE();
    case 605:
      if (lookahead == 't') ADVANCE(803);
      END_STATE();
    case 606:
      if (lookahead == 'r') ADVANCE(804);
      END_STATE();
    case 607:
      if (lookahead == 'g') ADVANCE(805);
      END_STATE();
    case 608:
      if (lookahead == 'x') ADVANCE(806);
      END_STATE();
    case 609:
      if (lookahead == 'a') ADVANCE(807);
      END_STATE();
    case 610:
      if (lookahead == 'e') ADVANCE(808);
      END_STATE();
    case 611:
      if (lookahead == 'a') ADVANCE(809);
      if (lookahead == 'o') ADVANCE(810);
      END_STATE();
    case 612:
      ACCEPT_TOKEN(sym_array);
      END_STATE();
    case 613:
      if (lookahead == 'r') ADVANCE(811);
      END_STATE();
    case 614:
      ACCEPT_TOKEN(anon_sym_async);
      END_STATE();
    case 615:
      if (lookahead == 'c') ADVANCE(812);
      END_STATE();
    case 616:
      if (lookahead == 'b') ADVANCE(813);
      END_STATE();
    case 617:
      ACCEPT_TOKEN(anon_sym_await);
      END_STATE();
    case 618:
      if (lookahead == 'e') ADVANCE(814);
      END_STATE();
    case 619:
      if (lookahead == 'n') ADVANCE(815);
      END_STATE();
    case 620:
      if (lookahead == 's') ADVANCE(816);
      END_STATE();
    case 621:
      ACCEPT_TOKEN(sym_break);
      END_STATE();
    case 622:
      ACCEPT_TOKEN(anon_sym_catch);
      END_STATE();
    case 623:
      if (lookahead == 'e') ADVANCE(817);
      END_STATE();
    case 624:
      if (lookahead == 'o') ADVANCE(818);
      END_STATE();
    case 625:
      ACCEPT_TOKEN(anon_sym_class);
      END_STATE();
    case 626:
      if (lookahead == 'a') ADVANCE(819);
      END_STATE();
    case 627:
      if (lookahead == 't') ADVANCE(820);
      END_STATE();
    case 628:
      if (lookahead == 'e') ADVANCE(821);
      END_STATE();
    case 629:
      if (lookahead == 'e') ADVANCE(822);
      END_STATE();
    case 630:
      if (lookahead == 'n') ADVANCE(823);
      END_STATE();
    case 631:
      if (lookahead == 'n') ADVANCE(824);
      END_STATE();
    case 632:
      if (lookahead == 'l') ADVANCE(825);
      END_STATE();
    case 633:
      if (lookahead == 'p') ADVANCE(826);
      END_STATE();
    case 634:
      ACCEPT_TOKEN(sym_const);
      if (lookahead == '_') ADVANCE(827);
      if (lookahead == 'e') ADVANCE(828);
      if (lookahead == 'i') ADVANCE(829);
      END_STATE();
    case 635:
      if (lookahead == 'n') ADVANCE(830);
      END_STATE();
    case 636:
      ACCEPT_TOKEN(anon_sym_crate);
      END_STATE();
    case 637:
      if (lookahead == 'g') ADVANCE(831);
      END_STATE();
    case 638:
      if (lookahead == 'y') ADVANCE(832);
      END_STATE();
    case 639:
      if (lookahead == 'l') ADVANCE(833);
      END_STATE();
    case 640:
      if (lookahead == 'e') ADVANCE(834);
      END_STATE();
    case 641:
      if (lookahead == 'e') ADVANCE(835);
      END_STATE();
    case 642:
      if (lookahead == 'r') ADVANCE(836);
      END_STATE();
    case 643:
      ACCEPT_TOKEN(anon_sym_dword);
      END_STATE();
    case 644:
      if (lookahead == 'i') ADVANCE(837);
      END_STATE();
    case 645:
      if (lookahead == 'e') ADVANCE(838);
      END_STATE();
    case 646:
      if (lookahead == 'c') ADVANCE(839);
      END_STATE();
    case 647:
      if (lookahead == 't') ADVANCE(840);
      END_STATE();
    case 648:
      if (lookahead == 'd') ADVANCE(841);
      END_STATE();
    case 649:
      if (lookahead == 'n') ADVANCE(842);
      END_STATE();
    case 650:
      if (lookahead == 'h') ADVANCE(843);
      END_STATE();
    case 651:
      ACCEPT_TOKEN(sym_false);
      END_STATE();
    case 652:
      if (lookahead == 'r') ADVANCE(844);
      END_STATE();
    case 653:
      ACCEPT_TOKEN(anon_sym_final);
      if (lookahead == 'l') ADVANCE(845);
      END_STATE();
    case 654:
      ACCEPT_TOKEN(anon_sym_fixed);
      END_STATE();
    case 655:
      if (lookahead == 'd') ADVANCE(846);
      END_STATE();
    case 656:
      if (lookahead == 'i') ADVANCE(847);
      END_STATE();
    case 657:
      ACCEPT_TOKEN(anon_sym_fvec2);
      END_STATE();
    case 658:
      ACCEPT_TOKEN(anon_sym_fvec3);
      END_STATE();
    case 659:
      ACCEPT_TOKEN(anon_sym_fvec4);
      END_STATE();
    case 660:
      if (lookahead == 'u') ADVANCE(848);
      END_STATE();
    case 661:
      if (lookahead == 's') ADVANCE(849);
      END_STATE();
    case 662:
      if (lookahead == 'e') ADVANCE(850);
      END_STATE();
    case 663:
      ACCEPT_TOKEN(anon_sym_highp);
      END_STATE();
    case 664:
      ACCEPT_TOKEN(anon_sym_hvec2);
      END_STATE();
    case 665:
      ACCEPT_TOKEN(anon_sym_hvec3);
      END_STATE();
    case 666:
      ACCEPT_TOKEN(anon_sym_hvec4);
      END_STATE();
    case 667:
      if (lookahead == 'e') ADVANCE(851);
      END_STATE();
    case 668:
      if (lookahead == '1') ADVANCE(852);
      if (lookahead == '2') ADVANCE(853);
      if (lookahead == '3') ADVANCE(854);
      if (lookahead == 'B') ADVANCE(855);
      if (lookahead == 'C') ADVANCE(856);
      END_STATE();
    case 669:
      if (lookahead == 'm') ADVANCE(857);
      END_STATE();
    case 670:
      if (lookahead == 't') ADVANCE(858);
      END_STATE();
    case 671:
      if (lookahead == 'e') ADVANCE(859);
      END_STATE();
    case 672:
      ACCEPT_TOKEN(anon_sym_inout);
      END_STATE();
    case 673:
      if (lookahead == 'n') ADVANCE(860);
      END_STATE();
    case 674:
      if (lookahead == 'f') ADVANCE(861);
      END_STATE();
    case 675:
      if (lookahead == 'i') ADVANCE(862);
      END_STATE();
    case 676:
      if (lookahead == 'l') ADVANCE(863);
      END_STATE();
    case 677:
      if (lookahead == 'a') ADVANCE(864);
      END_STATE();
    case 678:
      if (lookahead == 'u') ADVANCE(865);
      END_STATE();
    case 679:
      if (lookahead == 't') ADVANCE(866);
      END_STATE();
    case 680:
      if (lookahead == 'd') ADVANCE(867);
      if (lookahead == 'r') ADVANCE(868);
      END_STATE();
    case 681:
      ACCEPT_TOKEN(anon_sym_macro);
      if (lookahead == '_') ADVANCE(869);
      END_STATE();
    case 682:
      if (lookahead == '2') ADVANCE(870);
      if (lookahead == '3') ADVANCE(871);
      if (lookahead == '4') ADVANCE(872);
      END_STATE();
    case 683:
      if (lookahead == '2') ADVANCE(873);
      if (lookahead == '3') ADVANCE(874);
      if (lookahead == '4') ADVANCE(875);
      END_STATE();
    case 684:
      if (lookahead == '2') ADVANCE(876);
      if (lookahead == '3') ADVANCE(877);
      if (lookahead == '4') ADVANCE(878);
      END_STATE();
    case 685:
      ACCEPT_TOKEN(anon_sym_match);
      END_STATE();
    case 686:
      if (lookahead == 'x') ADVANCE(879);
      END_STATE();
    case 687:
      if (lookahead == 'm') ADVANCE(880);
      END_STATE();
    case 688:
      if (lookahead == 'e') ADVANCE(881);
      END_STATE();
    case 689:
      if (lookahead == 'l') ADVANCE(882);
      END_STATE();
    case 690:
      if (lookahead == 'p') ADVANCE(883);
      END_STATE();
    case 691:
      if (lookahead == 'e') ADVANCE(884);
      END_STATE();
    case 692:
      if (lookahead == 'i') ADVANCE(885);
      END_STATE();
    case 693:
      if (lookahead == 'e') ADVANCE(886);
      END_STATE();
    case 694:
      if (lookahead == 's') ADVANCE(887);
      END_STATE();
    case 695:
      if (lookahead == 't') ADVANCE(888);
      END_STATE();
    case 696:
      if (lookahead == 't') ADVANCE(889);
      END_STATE();
    case 697:
      if (lookahead == 'i') ADVANCE(890);
      END_STATE();
    case 698:
      if (lookahead == 'g') ADVANCE(891);
      END_STATE();
    case 699:
      if (lookahead == 'f') ADVANCE(892);
      END_STATE();
    case 700:
      if (lookahead == 't') ADVANCE(893);
      END_STATE();
    case 701:
      ACCEPT_TOKEN(anon_sym_patch);
      END_STATE();
    case 702:
      if (lookahead == 'f') ADVANCE(894);
      END_STATE();
    case 703:
      ACCEPT_TOKEN(anon_sym_point);
      END_STATE();
    case 704:
      if (lookahead == 's') ADVANCE(895);
      END_STATE();
    case 705:
      if (lookahead == 'r') ADVANCE(896);
      END_STATE();
    case 706:
      if (lookahead == 't') ADVANCE(897);
      END_STATE();
    case 707:
      if (lookahead == 'c') ADVANCE(898);
      END_STATE();
    case 708:
      if (lookahead == 'c') ADVANCE(899);
      END_STATE();
    case 709:
      if (lookahead == 'o') ADVANCE(900);
      END_STATE();
    case 710:
      if (lookahead == 'n') ADVANCE(901);
      END_STATE();
    case 711:
      if (lookahead == 'n') ADVANCE(902);
      END_STATE();
    case 712:
      if (lookahead == 'w') ADVANCE(903);
      END_STATE();
    case 713:
      if (lookahead == 'n') ADVANCE(904);
      END_STATE();
    case 714:
      if (lookahead == 'd') ADVANCE(905);
      END_STATE();
    case 715:
      if (lookahead == 't') ADVANCE(906);
      END_STATE();
    case 716:
      if (lookahead == 'e') ADVANCE(907);
      END_STATE();
    case 717:
      if (lookahead == 'r') ADVANCE(908);
      END_STATE();
    case 718:
      if (lookahead == 'r') ADVANCE(909);
      END_STATE();
    case 719:
      if (lookahead == 'i') ADVANCE(910);
      END_STATE();
    case 720:
      if (lookahead == 'n') ADVANCE(911);
      END_STATE();
    case 721:
      if (lookahead == 'l') ADVANCE(912);
      END_STATE();
    case 722:
      if (lookahead == 'i') ADVANCE(913);
      END_STATE();
    case 723:
      if (lookahead == 'i') ADVANCE(914);
      END_STATE();
    case 724:
      if (lookahead == '6') ADVANCE(915);
      END_STATE();
    case 725:
      if (lookahead == '2') ADVANCE(916);
      END_STATE();
    case 726:
      if (lookahead == 's') ADVANCE(917);
      if (lookahead == 'u') ADVANCE(918);
      END_STATE();
    case 727:
      if (lookahead == 'a') ADVANCE(919);
      END_STATE();
    case 728:
      if (lookahead == 'r') ADVANCE(920);
      END_STATE();
    case 729:
      if (lookahead == 'e') ADVANCE(921);
      END_STATE();
    case 730:
      if (lookahead == 'd') ADVANCE(922);
      END_STATE();
    case 731:
      if (lookahead == 'd') ADVANCE(923);
      END_STATE();
    case 732:
      if (lookahead == 'f') ADVANCE(924);
      END_STATE();
    case 733:
      if (lookahead == 'h') ADVANCE(925);
      END_STATE();
    case 734:
      ACCEPT_TOKEN(anon_sym_snorm);
      END_STATE();
    case 735:
      if (lookahead == 'b') ADVANCE(926);
      END_STATE();
    case 736:
      if (lookahead == 'c') ADVANCE(927);
      END_STATE();
    case 737:
      if (lookahead == 'g') ADVANCE(928);
      END_STATE();
    case 738:
      if (lookahead == 'g') ADVANCE(929);
      END_STATE();
    case 739:
      if (lookahead == 't') ADVANCE(930);
      END_STATE();
    case 740:
      if (lookahead == 's') ADVANCE(931);
      END_STATE();
    case 741:
      if (lookahead == 'u') ADVANCE(932);
      END_STATE();
    case 742:
      ACCEPT_TOKEN(anon_sym_super);
      if (lookahead == 'p') ADVANCE(933);
      END_STATE();
    case 743:
      if (lookahead == 'h') ADVANCE(934);
      END_STATE();
    case 744:
      if (lookahead == 't') ADVANCE(935);
      END_STATE();
    case 745:
      if (lookahead == 'e') ADVANCE(936);
      END_STATE();
    case 746:
      if (lookahead == 'i') ADVANCE(937);
      END_STATE();
    case 747:
      if (lookahead == 'a') ADVANCE(938);
      END_STATE();
    case 748:
      if (lookahead == 'r') ADVANCE(939);
      END_STATE();
    case 749:
      if (lookahead == 'd') ADVANCE(940);
      END_STATE();
    case 750:
      ACCEPT_TOKEN(anon_sym_throw);
      END_STATE();
    case 751:
      ACCEPT_TOKEN(anon_sym_trait);
      END_STATE();
    case 752:
      if (lookahead == 'g') ADVANCE(941);
      END_STATE();
    case 753:
      if (lookahead == 'e') ADVANCE(942);
      END_STATE();
    case 754:
      if (lookahead == 'd') ADVANCE(943);
      END_STATE();
    case 755:
      if (lookahead == 'a') ADVANCE(944);
      END_STATE();
    case 756:
      if (lookahead == 'f') ADVANCE(945);
      END_STATE();
    case 757:
      if (lookahead == 'e') ADVANCE(946);
      END_STATE();
    case 758:
      if (lookahead == 'r') ADVANCE(947);
      END_STATE();
    case 759:
      ACCEPT_TOKEN(anon_sym_union);
      END_STATE();
    case 760:
      if (lookahead == 's') ADVANCE(948);
      END_STATE();
    case 761:
      ACCEPT_TOKEN(anon_sym_unorm);
      END_STATE();
    case 762:
      if (lookahead == 'e') ADVANCE(949);
      END_STATE();
    case 763:
      if (lookahead == 'n') ADVANCE(950);
      END_STATE();
    case 764:
      if (lookahead == 'e') ADVANCE(951);
      END_STATE();
    case 765:
      if (lookahead == 'l') ADVANCE(952);
      END_STATE();
    case 766:
      ACCEPT_TOKEN(anon_sym_using);
      END_STATE();
    case 767:
      if (lookahead == 'a') ADVANCE(953);
      END_STATE();
    case 768:
      if (lookahead == 'u') ADVANCE(954);
      END_STATE();
    case 769:
      if (lookahead == 'n') ADVANCE(955);
      END_STATE();
    case 770:
      if (lookahead == 'r') ADVANCE(956);
      END_STATE();
    case 771:
      if (lookahead == 'x') ADVANCE(957);
      END_STATE();
    case 772:
      if (lookahead == 'a') ADVANCE(958);
      END_STATE();
    case 773:
      if (lookahead == 'i') ADVANCE(959);
      END_STATE();
    case 774:
      if (lookahead == '_') ADVANCE(960);
      END_STATE();
    case 775:
      ACCEPT_TOKEN(anon_sym_where);
      END_STATE();
    case 776:
      ACCEPT_TOKEN(sym_while);
      END_STATE();
    case 777:
      if (lookahead == 'r') ADVANCE(961);
      END_STATE();
    case 778:
      ACCEPT_TOKEN(anon_sym_write);
      if (lookahead == 'o') ADVANCE(962);
      END_STATE();
    case 779:
      ACCEPT_TOKEN(anon_sym_yield);
      END_STATE();
    case 780:
      if (lookahead == 'S') ADVANCE(963);
      END_STATE();
    case 781:
      if (lookahead == 't') ADVANCE(964);
      END_STATE();
    case 782:
      ACCEPT_TOKEN(anon_sym_Buffer);
      END_STATE();
    case 783:
      if (lookahead == 'd') ADVANCE(965);
      END_STATE();
    case 784:
      if (lookahead == 'e') ADVANCE(966);
      END_STATE();
    case 785:
      if (lookahead == 'e') ADVANCE(967);
      END_STATE();
    case 786:
      if (lookahead == 'e') ADVANCE(968);
      END_STATE();
    case 787:
      if (lookahead == 't') ADVANCE(969);
      END_STATE();
    case 788:
      if (lookahead == 'S') ADVANCE(970);
      END_STATE();
    case 789:
      if (lookahead == 'r') ADVANCE(971);
      END_STATE();
    case 790:
      if (lookahead == 'a') ADVANCE(972);
      END_STATE();
    case 791:
      if (lookahead == 'a') ADVANCE(973);
      END_STATE();
    case 792:
      if (lookahead == 'r') ADVANCE(974);
      END_STATE();
    case 793:
      if (lookahead == 'P') ADVANCE(975);
      END_STATE();
    case 794:
      if (lookahead == 'h') ADVANCE(976);
      END_STATE();
    case 795:
      if (lookahead == 't') ADVANCE(977);
      END_STATE();
    case 796:
      if (lookahead == 'e') ADVANCE(978);
      END_STATE();
    case 797:
      if (lookahead == 'A') ADVANCE(979);
      END_STATE();
    case 798:
      if (lookahead == 'c') ADVANCE(980);
      END_STATE();
    case 799:
      if (lookahead == 'u') ADVANCE(981);
      END_STATE();
    case 800:
      if (lookahead == 'i') ADVANCE(982);
      END_STATE();
    case 801:
      if (lookahead == 'T') ADVANCE(983);
      END_STATE();
    case 802:
      if (lookahead == 'r') ADVANCE(984);
      END_STATE();
    case 803:
      if (lookahead == 'u') ADVANCE(985);
      END_STATE();
    case 804:
      if (lookahead == 'e') ADVANCE(986);
      END_STATE();
    case 805:
      if (lookahead == 'l') ADVANCE(987);
      END_STATE();
    case 806:
      if (lookahead == 'S') ADVANCE(988);
      END_STATE();
    case 807:
      if (lookahead == 'c') ADVANCE(989);
      END_STATE();
    case 808:
      ACCEPT_TOKEN(anon_sym_active);
      END_STATE();
    case 809:
      if (lookahead == 's') ADVANCE(990);
      END_STATE();
    case 810:
      if (lookahead == 'f') ADVANCE(991);
      END_STATE();
    case 811:
      if (lookahead == 'a') ADVANCE(992);
      END_STATE();
    case 812:
      ACCEPT_TOKEN(sym_atomic);
      if (lookahead == '_') ADVANCE(993);
      END_STATE();
    case 813:
      if (lookahead == 'u') ADVANCE(994);
      END_STATE();
    case 814:
      ACCEPT_TOKEN(anon_sym_become);
      END_STATE();
    case 815:
      if (lookahead == 'g') ADVANCE(995);
      END_STATE();
    case 816:
      if (lookahead == 't') ADVANCE(996);
      END_STATE();
    case 817:
      if (lookahead == 'r') ADVANCE(997);
      END_STATE();
    case 818:
      if (lookahead == 'i') ADVANCE(998);
      END_STATE();
    case 819:
      if (lookahead == 'i') ADVANCE(999);
      END_STATE();
    case 820:
      if (lookahead == 'u') ADVANCE(1000);
      END_STATE();
    case 821:
      if (lookahead == 'l') ADVANCE(1001);
      END_STATE();
    case 822:
      if (lookahead == 'n') ADVANCE(1002);
      END_STATE();
    case 823:
      if (lookahead == '_') ADVANCE(1003);
      END_STATE();
    case 824:
      ACCEPT_TOKEN(anon_sym_common);
      END_STATE();
    case 825:
      if (lookahead == 'e') ADVANCE(1004);
      END_STATE();
    case 826:
      if (lookahead == 't') ADVANCE(1005);
      END_STATE();
    case 827:
      if (lookahead == 'c') ADVANCE(1006);
      END_STATE();
    case 828:
      if (lookahead == 'v') ADVANCE(1007);
      if (lookahead == 'x') ADVANCE(1008);
      END_STATE();
    case 829:
      if (lookahead == 'n') ADVANCE(1009);
      END_STATE();
    case 830:
      if (lookahead == 'u') ADVANCE(1010);
      END_STATE();
    case 831:
      if (lookahead == 'e') ADVANCE(1011);
      END_STATE();
    case 832:
      if (lookahead == 'p') ADVANCE(1012);
      END_STATE();
    case 833:
      if (lookahead == 't') ADVANCE(1013);
      END_STATE();
    case 834:
      ACCEPT_TOKEN(anon_sym_delete);
      END_STATE();
    case 835:
      ACCEPT_TOKEN(anon_sym_demote);
      if (lookahead == '_') ADVANCE(1014);
      END_STATE();
    case 836:
      if (lookahead == 'd') ADVANCE(1015);
      END_STATE();
    case 837:
      if (lookahead == 'c') ADVANCE(1016);
      END_STATE();
    case 838:
      ACCEPT_TOKEN(sym_enable);
      END_STATE();
    case 839:
      if (lookahead == 'i') ADVANCE(1017);
      END_STATE();
    case 840:
      ACCEPT_TOKEN(anon_sym_export);
      END_STATE();
    case 841:
      if (lookahead == 's') ADVANCE(1018);
      END_STATE();
    case 842:
      ACCEPT_TOKEN(anon_sym_extern);
      if (lookahead == 'a') ADVANCE(1019);
      END_STATE();
    case 843:
      if (lookahead == 'r') ADVANCE(1020);
      END_STATE();
    case 844:
      ACCEPT_TOKEN(anon_sym_filter);
      END_STATE();
    case 845:
      if (lookahead == 'y') ADVANCE(1021);
      END_STATE();
    case 846:
      ACCEPT_TOKEN(anon_sym_friend);
      END_STATE();
    case 847:
      if (lookahead == 'o') ADVANCE(1022);
      END_STATE();
    case 848:
      if (lookahead == 'p') ADVANCE(1023);
      END_STATE();
    case 849:
      if (lookahead == 'h') ADVANCE(1024);
      END_STATE();
    case 850:
      ACCEPT_TOKEN(anon_sym_handle);
      END_STATE();
    case 851:
      if (lookahead == '1') ADVANCE(1025);
      if (lookahead == '2') ADVANCE(1026);
      if (lookahead == '3') ADVANCE(1027);
      if (lookahead == 'B') ADVANCE(1028);
      if (lookahead == 'C') ADVANCE(1029);
      END_STATE();
    case 852:
      if (lookahead == 'D') ADVANCE(1030);
      END_STATE();
    case 853:
      if (lookahead == 'D') ADVANCE(1031);
      END_STATE();
    case 854:
      if (lookahead == 'D') ADVANCE(1032);
      END_STATE();
    case 855:
      if (lookahead == 'u') ADVANCE(1033);
      END_STATE();
    case 856:
      if (lookahead == 'u') ADVANCE(1034);
      END_STATE();
    case 857:
      if (lookahead == 'e') ADVANCE(1035);
      END_STATE();
    case 858:
      ACCEPT_TOKEN(anon_sym_import);
      END_STATE();
    case 859:
      ACCEPT_TOKEN(anon_sym_inline);
      END_STATE();
    case 860:
      if (lookahead == 'c') ADVANCE(1036);
      END_STATE();
    case 861:
      if (lookahead == 'a') ADVANCE(1037);
      END_STATE();
    case 862:
      if (lookahead == 'a') ADVANCE(1038);
      END_STATE();
    case 863:
      if (lookahead == 'e') ADVANCE(1039);
      END_STATE();
    case 864:
      if (lookahead == 's') ADVANCE(1040);
      END_STATE();
    case 865:
      if (lookahead == 'r') ADVANCE(1041);
      END_STATE();
    case 866:
      ACCEPT_TOKEN(anon_sym_layout);
      END_STATE();
    case 867:
      if (lookahead == 'j') ADVANCE(1042);
      END_STATE();
    case 868:
      ACCEPT_TOKEN(anon_sym_linear);
      END_STATE();
    case 869:
      if (lookahead == 'r') ADVANCE(1043);
      END_STATE();
    case 870:
      ACCEPT_TOKEN(sym_mat2x2);
      END_STATE();
    case 871:
      ACCEPT_TOKEN(sym_mat2x3);
      END_STATE();
    case 872:
      ACCEPT_TOKEN(sym_mat2x4);
      END_STATE();
    case 873:
      ACCEPT_TOKEN(sym_mat3x2);
      END_STATE();
    case 874:
      ACCEPT_TOKEN(sym_mat3x3);
      END_STATE();
    case 875:
      ACCEPT_TOKEN(sym_mat3x4);
      END_STATE();
    case 876:
      ACCEPT_TOKEN(sym_mat4x2);
      END_STATE();
    case 877:
      ACCEPT_TOKEN(sym_mat4x3);
      END_STATE();
    case 878:
      ACCEPT_TOKEN(sym_mat4x4);
      END_STATE();
    case 879:
      ACCEPT_TOKEN(anon_sym_matrix);
      END_STATE();
    case 880:
      if (lookahead == 'p') ADVANCE(1044);
      END_STATE();
    case 881:
      ACCEPT_TOKEN(anon_sym_module);
      END_STATE();
    case 882:
      if (lookahead == 'e') ADVANCE(1045);
      END_STATE();
    case 883:
      if (lookahead == 'a') ADVANCE(1046);
      END_STATE();
    case 884:
      if (lookahead == 'p') ADVANCE(1047);
      END_STATE();
    case 885:
      if (lookahead == 'n') ADVANCE(1048);
      END_STATE();
    case 886:
      if (lookahead == 'r') ADVANCE(1049);
      END_STATE();
    case 887:
      if (lookahead == 'p') ADVANCE(1050);
      END_STATE();
    case 888:
      if (lookahead == 'r') ADVANCE(1051);
      END_STATE();
    case 889:
      if (lookahead == 'o') ADVANCE(1052);
      END_STATE();
    case 890:
      if (lookahead == 'd') ADVANCE(1053);
      END_STATE();
    case 891:
      if (lookahead == 'e') ADVANCE(1054);
      END_STATE();
    case 892:
      if (lookahead == 'f') ADVANCE(1055);
      END_STATE();
    case 893:
      if (lookahead == 'i') ADVANCE(1056);
      END_STATE();
    case 894:
      if (lookahead == 'r') ADVANCE(1057);
      END_STATE();
    case 895:
      if (lookahead == 'e') ADVANCE(1058);
      if (lookahead == 'i') ADVANCE(1059);
      END_STATE();
    case 896:
      if (lookahead == 'g') ADVANCE(1060);
      END_STATE();
    case 897:
      if (lookahead == 'e') ADVANCE(1061);
      END_STATE();
    case 898:
      if (lookahead == 't') ADVANCE(1062);
      END_STATE();
    case 899:
      ACCEPT_TOKEN(anon_sym_public);
      END_STATE();
    case 900:
      if (lookahead == 'a') ADVANCE(1063);
      END_STATE();
    case 901:
      if (lookahead == 't') ADVANCE(1064);
      END_STATE();
    case 902:
      if (lookahead == 't') ADVANCE(1065);
      END_STATE();
    case 903:
      if (lookahead == 'r') ADVANCE(1066);
      END_STATE();
    case 904:
      if (lookahead == 'l') ADVANCE(1067);
      END_STATE();
    case 905:
      if (lookahead == 'l') ADVANCE(1068);
      END_STATE();
    case 906:
      if (lookahead == 'e') ADVANCE(1069);
      END_STATE();
    case 907:
      if (lookahead == 'r') ADVANCE(1070);
      END_STATE();
    case 908:
      if (lookahead == 'e') ADVANCE(1071);
      END_STATE();
    case 909:
      if (lookahead == 'c') ADVANCE(1072);
      END_STATE();
    case 910:
      if (lookahead == 'c') ADVANCE(1073);
      END_STATE();
    case 911:
      ACCEPT_TOKEN(sym_return);
      END_STATE();
    case 912:
      if (lookahead == 'o') ADVANCE(1074);
      END_STATE();
    case 913:
      if (lookahead == 'n') ADVANCE(1075);
      END_STATE();
    case 914:
      if (lookahead == 'n') ADVANCE(1076);
      END_STATE();
    case 915:
      if (lookahead == 'f') ADVANCE(1077);
      if (lookahead == 's') ADVANCE(1078);
      if (lookahead == 'u') ADVANCE(1079);
      END_STATE();
    case 916:
      if (lookahead == 'f') ADVANCE(1080);
      if (lookahead == 's') ADVANCE(1081);
      if (lookahead == 'u') ADVANCE(1082);
      END_STATE();
    case 917:
      if (lookahead == 'i') ADVANCE(1083);
      if (lookahead == 'n') ADVANCE(1084);
      END_STATE();
    case 918:
      if (lookahead == 'i') ADVANCE(1085);
      if (lookahead == 'n') ADVANCE(1086);
      END_STATE();
    case 919:
      if (lookahead == 'j') ADVANCE(1087);
      END_STATE();
    case 920:
      ACCEPT_TOKEN(anon_sym_samper);
      END_STATE();
    case 921:
      ACCEPT_TOKEN(anon_sym_sample);
      if (lookahead == 'r') ADVANCE(1088);
      END_STATE();
    case 922:
      ACCEPT_TOKEN(anon_sym_shared);
      END_STATE();
    case 923:
      ACCEPT_TOKEN(anon_sym_signed);
      END_STATE();
    case 924:
      ACCEPT_TOKEN(anon_sym_sizeof);
      END_STATE();
    case 925:
      ACCEPT_TOKEN(anon_sym_smooth);
      END_STATE();
    case 926:
      if (lookahead == 'l') ADVANCE(1089);
      END_STATE();
    case 927:
      ACCEPT_TOKEN(anon_sym_static);
      if (lookahead == 'A') ADVANCE(1090);
      if (lookahead == '_') ADVANCE(1091);
      END_STATE();
    case 928:
      if (lookahead == 'e') ADVANCE(1092);
      END_STATE();
    case 929:
      ACCEPT_TOKEN(anon_sym_string);
      END_STATE();
    case 930:
      ACCEPT_TOKEN(sym_struct);
      END_STATE();
    case 931:
      if (lookahead == 's') ADVANCE(1093);
      END_STATE();
    case 932:
      if (lookahead == 't') ADVANCE(1094);
      END_STATE();
    case 933:
      ACCEPT_TOKEN(anon_sym_superp);
      END_STATE();
    case 934:
      ACCEPT_TOKEN(sym_switch);
      END_STATE();
    case 935:
      ACCEPT_TOKEN(anon_sym_target);
      END_STATE();
    case 936:
      if (lookahead == 'r') ADVANCE(1095);
      END_STATE();
    case 937:
      if (lookahead == 'q') ADVANCE(1096);
      END_STATE();
    case 938:
      if (lookahead == 't') ADVANCE(1097);
      END_STATE();
    case 939:
      if (lookahead == 'e') ADVANCE(1098);
      END_STATE();
    case 940:
      if (lookahead == '_') ADVANCE(1099);
      END_STATE();
    case 941:
      if (lookahead == 'l') ADVANCE(1100);
      END_STATE();
    case 942:
      if (lookahead == 'f') ADVANCE(1101);
      END_STATE();
    case 943:
      ACCEPT_TOKEN(anon_sym_typeid);
      END_STATE();
    case 944:
      if (lookahead == 'm') ADVANCE(1102);
      END_STATE();
    case 945:
      ACCEPT_TOKEN(anon_sym_typeof);
      END_STATE();
    case 946:
      if (lookahead == '1') ADVANCE(1103);
      if (lookahead == '2') ADVANCE(1104);
      if (lookahead == '3') ADVANCE(1105);
      if (lookahead == 'B') ADVANCE(1106);
      if (lookahead == 'C') ADVANCE(1107);
      END_STATE();
    case 947:
      if (lookahead == 'm') ADVANCE(1108);
      END_STATE();
    case 948:
      ACCEPT_TOKEN(anon_sym_unless);
      END_STATE();
    case 949:
      ACCEPT_TOKEN(anon_sym_unsafe);
      END_STATE();
    case 950:
      if (lookahead == 'e') ADVANCE(1109);
      END_STATE();
    case 951:
      if (lookahead == 'd') ADVANCE(1110);
      END_STATE();
    case 952:
      if (lookahead == 'e') ADVANCE(1111);
      END_STATE();
    case 953:
      if (lookahead == 's') ADVANCE(1112);
      END_STATE();
    case 954:
      if (lookahead == 'r') ADVANCE(1113);
      END_STATE();
    case 955:
      if (lookahead == 'g') ADVANCE(1114);
      END_STATE();
    case 956:
      ACCEPT_TOKEN(anon_sym_vector);
      END_STATE();
    case 957:
      if (lookahead == 'f') ADVANCE(1115);
      END_STATE();
    case 958:
      if (lookahead == 'l') ADVANCE(1116);
      END_STATE();
    case 959:
      if (lookahead == 'l') ADVANCE(1117);
      END_STATE();
    case 960:
      if (lookahead == 't') ADVANCE(1118);
      END_STATE();
    case 961:
      if (lookahead == 'o') ADVANCE(1119);
      END_STATE();
    case 962:
      if (lookahead == 'n') ADVANCE(1120);
      END_STATE();
    case 963:
      if (lookahead == 't') ADVANCE(1121);
      END_STATE();
    case 964:
      if (lookahead == 'a') ADVANCE(1122);
      END_STATE();
    case 965:
      if (lookahead == 'r') ADVANCE(1123);
      END_STATE();
    case 966:
      if (lookahead == 'S') ADVANCE(1124);
      END_STATE();
    case 967:
      if (lookahead == 'S') ADVANCE(1125);
      END_STATE();
    case 968:
      if (lookahead == 'S') ADVANCE(1126);
      END_STATE();
    case 969:
      if (lookahead == 'e') ADVANCE(1127);
      END_STATE();
    case 970:
      if (lookahead == 'h') ADVANCE(1128);
      END_STATE();
    case 971:
      if (lookahead == 'y') ADVANCE(1129);
      END_STATE();
    case 972:
      if (lookahead == 'd') ADVANCE(1130);
      END_STATE();
    case 973:
      if (lookahead == 't') ADVANCE(1131);
      END_STATE();
    case 974:
      if (lookahead == 'e') ADVANCE(1132);
      END_STATE();
    case 975:
      if (lookahead == 'a') ADVANCE(1133);
      END_STATE();
    case 976:
      if (lookahead == 'a') ADVANCE(1134);
      END_STATE();
    case 977:
      if (lookahead == 'r') ADVANCE(1135);
      END_STATE();
    case 978:
      if (lookahead == 'r') ADVANCE(1136);
      END_STATE();
    case 979:
      if (lookahead == 'd') ADVANCE(1137);
      END_STATE();
    case 980:
      if (lookahead == 't') ADVANCE(1138);
      END_STATE();
    case 981:
      if (lookahead == 'r') ADVANCE(1139);
      END_STATE();
    case 982:
      if (lookahead == 'z') ADVANCE(1140);
      END_STATE();
    case 983:
      if (lookahead == 'a') ADVANCE(1141);
      END_STATE();
    case 984:
      if (lookahead == 'C') ADVANCE(1142);
      if (lookahead == 'S') ADVANCE(1143);
      END_STATE();
    case 985:
      if (lookahead == 'r') ADVANCE(1144);
      END_STATE();
    case 986:
      if (lookahead == '1') ADVANCE(1145);
      if (lookahead == '2') ADVANCE(1146);
      if (lookahead == '3') ADVANCE(1147);
      if (lookahead == 'C') ADVANCE(1148);
      END_STATE();
    case 987:
      if (lookahead == 'e') ADVANCE(1149);
      END_STATE();
    case 988:
      if (lookahead == 'h') ADVANCE(1150);
      END_STATE();
    case 989:
      if (lookahead == 't') ADVANCE(1151);
      END_STATE();
    case 990:
      ACCEPT_TOKEN(anon_sym_alignas);
      END_STATE();
    case 991:
      ACCEPT_TOKEN(anon_sym_alignof);
      END_STATE();
    case 992:
      if (lookahead == 'g') ADVANCE(1152);
      END_STATE();
    case 993:
      if (lookahead == 'u') ADVANCE(1153);
      END_STATE();
    case 994:
      if (lookahead == 't') ADVANCE(1154);
      END_STATE();
    case 995:
      if (lookahead == '_') ADVANCE(1155);
      END_STATE();
    case 996:
      ACCEPT_TOKEN(sym_bitcast);
      END_STATE();
    case 997:
      ACCEPT_TOKEN(anon_sym_cbuffer);
      END_STATE();
    case 998:
      if (lookahead == 'd') ADVANCE(1156);
      END_STATE();
    case 999:
      if (lookahead == 't') ADVANCE(1157);
      END_STATE();
    case 1000:
      if (lookahead == 'r') ADVANCE(1158);
      END_STATE();
    case 1001:
      if (lookahead == 'd') ADVANCE(1159);
      END_STATE();
    case 1002:
      if (lookahead == 't') ADVANCE(1160);
      END_STATE();
    case 1003:
      if (lookahead == 'm') ADVANCE(1161);
      END_STATE();
    case 1004:
      ACCEPT_TOKEN(anon_sym_compile);
      if (lookahead == '_') ADVANCE(1162);
      END_STATE();
    case 1005:
      ACCEPT_TOKEN(anon_sym_concept);
      END_STATE();
    case 1006:
      if (lookahead == 'a') ADVANCE(1163);
      END_STATE();
    case 1007:
      if (lookahead == 'a') ADVANCE(1164);
      END_STATE();
    case 1008:
      if (lookahead == 'p') ADVANCE(1165);
      END_STATE();
    case 1009:
      if (lookahead == 'i') ADVANCE(1166);
      END_STATE();
    case 1010:
      if (lookahead == 'e') ADVANCE(1167);
      if (lookahead == 'i') ADVANCE(1168);
      END_STATE();
    case 1011:
      if (lookahead == 'r') ADVANCE(1169);
      END_STATE();
    case 1012:
      if (lookahead == 'e') ADVANCE(1170);
      END_STATE();
    case 1013:
      ACCEPT_TOKEN(sym_default);
      END_STATE();
    case 1014:
      if (lookahead == 't') ADVANCE(1171);
      END_STATE();
    case 1015:
      ACCEPT_TOKEN(sym_discard);
      END_STATE();
    case 1016:
      if (lookahead == '_') ADVANCE(1172);
      END_STATE();
    case 1017:
      if (lookahead == 't') ADVANCE(1173);
      END_STATE();
    case 1018:
      ACCEPT_TOKEN(anon_sym_extends);
      END_STATE();
    case 1019:
      if (lookahead == 'l') ADVANCE(1174);
      END_STATE();
    case 1020:
      if (lookahead == 'o') ADVANCE(1175);
      END_STATE();
    case 1021:
      ACCEPT_TOKEN(anon_sym_finally);
      END_STATE();
    case 1022:
      if (lookahead == 'n') ADVANCE(1176);
      END_STATE();
    case 1023:
      ACCEPT_TOKEN(anon_sym_fxgroup);
      END_STATE();
    case 1024:
      if (lookahead == 'a') ADVANCE(1177);
      END_STATE();
    case 1025:
      if (lookahead == 'D') ADVANCE(1178);
      END_STATE();
    case 1026:
      if (lookahead == 'D') ADVANCE(1179);
      END_STATE();
    case 1027:
      if (lookahead == 'D') ADVANCE(1180);
      END_STATE();
    case 1028:
      if (lookahead == 'u') ADVANCE(1181);
      END_STATE();
    case 1029:
      if (lookahead == 'u') ADVANCE(1182);
      END_STATE();
    case 1030:
      ACCEPT_TOKEN(anon_sym_image1D);
      if (lookahead == 'A') ADVANCE(1183);
      END_STATE();
    case 1031:
      ACCEPT_TOKEN(anon_sym_image2D);
      if (lookahead == 'A') ADVANCE(1184);
      if (lookahead == 'M') ADVANCE(1185);
      if (lookahead == 'R') ADVANCE(1186);
      END_STATE();
    case 1032:
      ACCEPT_TOKEN(anon_sym_image3D);
      END_STATE();
    case 1033:
      if (lookahead == 'f') ADVANCE(1187);
      END_STATE();
    case 1034:
      if (lookahead == 'b') ADVANCE(1188);
      END_STATE();
    case 1035:
      if (lookahead == 'n') ADVANCE(1189);
      END_STATE();
    case 1036:
      if (lookahead == 'e') ADVANCE(1190);
      END_STATE();
    case 1037:
      if (lookahead == 'c') ADVANCE(1191);
      END_STATE();
    case 1038:
      if (lookahead == 'n') ADVANCE(1192);
      END_STATE();
    case 1039:
      if (lookahead == 'r') ADVANCE(1193);
      END_STATE();
    case 1040:
      if (lookahead == 's') ADVANCE(1194);
      END_STATE();
    case 1041:
      if (lookahead == 'e') ADVANCE(1195);
      END_STATE();
    case 1042:
      ACCEPT_TOKEN(anon_sym_lineadj);
      END_STATE();
    case 1043:
      if (lookahead == 'u') ADVANCE(1196);
      END_STATE();
    case 1044:
      ACCEPT_TOKEN(anon_sym_mediump);
      END_STATE();
    case 1045:
      ACCEPT_TOKEN(anon_sym_mutable);
      END_STATE();
    case 1046:
      if (lookahead == 'c') ADVANCE(1197);
      END_STATE();
    case 1047:
      if (lookahead == 't') ADVANCE(1198);
      END_STATE();
    case 1048:
      if (lookahead == 'e') ADVANCE(1199);
      END_STATE();
    case 1049:
      if (lookahead == 'p') ADVANCE(1200);
      END_STATE();
    case 1050:
      if (lookahead == 'e') ADVANCE(1201);
      END_STATE();
    case 1051:
      ACCEPT_TOKEN(anon_sym_nullptr);
      END_STATE();
    case 1052:
      if (lookahead == 'r') ADVANCE(1202);
      END_STATE();
    case 1053:
      if (lookahead == 'e') ADVANCE(1203);
      END_STATE();
    case 1054:
      ACCEPT_TOKEN(anon_sym_package);
      END_STATE();
    case 1055:
      if (lookahead == 's') ADVANCE(1204);
      END_STATE();
    case 1056:
      if (lookahead == 'o') ADVANCE(1205);
      END_STATE();
    case 1057:
      if (lookahead == 'a') ADVANCE(1206);
      END_STATE();
    case 1058:
      ACCEPT_TOKEN(anon_sym_precise);
      END_STATE();
    case 1059:
      if (lookahead == 'o') ADVANCE(1207);
      END_STATE();
    case 1060:
      if (lookahead == 'e') ADVANCE(1208);
      END_STATE();
    case 1061:
      ACCEPT_TOKEN(sym_private);
      END_STATE();
    case 1062:
      if (lookahead == 'e') ADVANCE(1209);
      END_STATE();
    case 1063:
      if (lookahead == 't') ADVANCE(1210);
      END_STATE();
    case 1064:
      ACCEPT_TOKEN(anon_sym_r32sint);
      END_STATE();
    case 1065:
      ACCEPT_TOKEN(anon_sym_r32uint);
      END_STATE();
    case 1066:
      if (lookahead == 'i') ADVANCE(1211);
      END_STATE();
    case 1067:
      if (lookahead == 'y') ADVANCE(1212);
      END_STATE();
    case 1068:
      if (lookahead == 'e') ADVANCE(1213);
      END_STATE();
    case 1069:
      if (lookahead == 'r') ADVANCE(1214);
      END_STATE();
    case 1070:
      if (lookahead == 'p') ADVANCE(1215);
      END_STATE();
    case 1071:
      if (lookahead == 's') ADVANCE(1216);
      END_STATE();
    case 1072:
      if (lookahead == 'e') ADVANCE(1217);
      END_STATE();
    case 1073:
      if (lookahead == 't') ADVANCE(1218);
      END_STATE();
    case 1074:
      if (lookahead == 'a') ADVANCE(1219);
      END_STATE();
    case 1075:
      if (lookahead == 't') ADVANCE(1220);
      END_STATE();
    case 1076:
      if (lookahead == 't') ADVANCE(1221);
      END_STATE();
    case 1077:
      if (lookahead == 'l') ADVANCE(1222);
      END_STATE();
    case 1078:
      if (lookahead == 'i') ADVANCE(1223);
      END_STATE();
    case 1079:
      if (lookahead == 'i') ADVANCE(1224);
      END_STATE();
    case 1080:
      if (lookahead == 'l') ADVANCE(1225);
      END_STATE();
    case 1081:
      if (lookahead == 'i') ADVANCE(1226);
      END_STATE();
    case 1082:
      if (lookahead == 'i') ADVANCE(1227);
      END_STATE();
    case 1083:
      if (lookahead == 'n') ADVANCE(1228);
      END_STATE();
    case 1084:
      if (lookahead == 'o') ADVANCE(1229);
      END_STATE();
    case 1085:
      if (lookahead == 'n') ADVANCE(1230);
      END_STATE();
    case 1086:
      if (lookahead == 'o') ADVANCE(1231);
      END_STATE();
    case 1087:
      if (lookahead == 'o') ADVANCE(1232);
      END_STATE();
    case 1088:
      ACCEPT_TOKEN(sym_sampler);
      if (lookahead == '1') ADVANCE(1233);
      if (lookahead == '2') ADVANCE(1234);
      if (lookahead == '3') ADVANCE(1235);
      if (lookahead == 'B') ADVANCE(1236);
      if (lookahead == 'C') ADVANCE(1237);
      if (lookahead == 'S') ADVANCE(1238);
      if (lookahead == '_') ADVANCE(1239);
      END_STATE();
    case 1089:
      if (lookahead == 'o') ADVANCE(1240);
      END_STATE();
    case 1090:
      if (lookahead == 's') ADVANCE(1241);
      END_STATE();
    case 1091:
      if (lookahead == 'a') ADVANCE(1242);
      if (lookahead == 'c') ADVANCE(1243);
      END_STATE();
    case 1092:
      ACCEPT_TOKEN(sym_storage);
      END_STATE();
    case 1093:
      if (lookahead == 'I') ADVANCE(1244);
      END_STATE();
    case 1094:
      if (lookahead == 'i') ADVANCE(1245);
      END_STATE();
    case 1095:
      ACCEPT_TOKEN(anon_sym_tbuffer);
      END_STATE();
    case 1096:
      if (lookahead == 'u') ADVANCE(1246);
      END_STATE();
    case 1097:
      if (lookahead == 'e') ADVANCE(1247);
      END_STATE();
    case 1098:
      if (lookahead == '1') ADVANCE(1248);
      if (lookahead == '2') ADVANCE(1249);
      if (lookahead == '3') ADVANCE(1250);
      if (lookahead == 'B') ADVANCE(1251);
      if (lookahead == 'C') ADVANCE(1252);
      if (lookahead == '_') ADVANCE(1253);
      END_STATE();
    case 1099:
      if (lookahead == 'l') ADVANCE(1254);
      END_STATE();
    case 1100:
      if (lookahead == 'e') ADVANCE(1255);
      END_STATE();
    case 1101:
      ACCEPT_TOKEN(anon_sym_typedef);
      END_STATE();
    case 1102:
      if (lookahead == 'e') ADVANCE(1256);
      END_STATE();
    case 1103:
      if (lookahead == 'D') ADVANCE(1257);
      END_STATE();
    case 1104:
      if (lookahead == 'D') ADVANCE(1258);
      END_STATE();
    case 1105:
      if (lookahead == 'D') ADVANCE(1259);
      END_STATE();
    case 1106:
      if (lookahead == 'u') ADVANCE(1260);
      END_STATE();
    case 1107:
      if (lookahead == 'u') ADVANCE(1261);
      END_STATE();
    case 1108:
      ACCEPT_TOKEN(sym_uniform);
      END_STATE();
    case 1109:
      if (lookahead == 'd') ADVANCE(1262);
      END_STATE();
    case 1110:
      ACCEPT_TOKEN(anon_sym_unsized);
      END_STATE();
    case 1111:
      if (lookahead == 'r') ADVANCE(1263);
      END_STATE();
    case 1112:
      if (lookahead == 's') ADVANCE(1264);
      END_STATE();
    case 1113:
      if (lookahead == 'e') ADVANCE(1265);
      END_STATE();
    case 1114:
      ACCEPT_TOKEN(anon_sym_varying);
      END_STATE();
    case 1115:
      if (lookahead == 'r') ADVANCE(1266);
      END_STATE();
    case 1116:
      ACCEPT_TOKEN(anon_sym_virtual);
      END_STATE();
    case 1117:
      if (lookahead == 'e') ADVANCE(1267);
      END_STATE();
    case 1118:
      ACCEPT_TOKEN(anon_sym_wchar_t);
      END_STATE();
    case 1119:
      if (lookahead == 'u') ADVANCE(1268);
      END_STATE();
    case 1120:
      if (lookahead == 'l') ADVANCE(1269);
      END_STATE();
    case 1121:
      if (lookahead == 'r') ADVANCE(1270);
      END_STATE();
    case 1122:
      if (lookahead == 't') ADVANCE(1271);
      END_STATE();
    case 1123:
      if (lookahead == 'e') ADVANCE(1272);
      END_STATE();
    case 1124:
      if (lookahead == 'h') ADVANCE(1273);
      END_STATE();
    case 1125:
      if (lookahead == 'h') ADVANCE(1274);
      END_STATE();
    case 1126:
      if (lookahead == 't') ADVANCE(1275);
      END_STATE();
    case 1127:
      if (lookahead == 'n') ADVANCE(1276);
      END_STATE();
    case 1128:
      if (lookahead == 'a') ADVANCE(1277);
      END_STATE();
    case 1129:
      if (lookahead == 'S') ADVANCE(1278);
      END_STATE();
    case 1130:
      if (lookahead == 'e') ADVANCE(1279);
      END_STATE();
    case 1131:
      if (lookahead == 'c') ADVANCE(1280);
      END_STATE();
    case 1132:
      if (lookahead == 'a') ADVANCE(1281);
      END_STATE();
    case 1133:
      if (lookahead == 't') ADVANCE(1282);
      END_STATE();
    case 1134:
      if (lookahead == 'd') ADVANCE(1283);
      END_STATE();
    case 1135:
      if (lookahead == 'e') ADVANCE(1284);
      END_STATE();
    case 1136:
      ACCEPT_TOKEN(anon_sym_RWBuffer);
      END_STATE();
    case 1137:
      if (lookahead == 'd') ADVANCE(1285);
      END_STATE();
    case 1138:
      if (lookahead == 'u') ADVANCE(1286);
      END_STATE();
    case 1139:
      if (lookahead == 'e') ADVANCE(1287);
      END_STATE();
    case 1140:
      if (lookahead == 'e') ADVANCE(1288);
      END_STATE();
    case 1141:
      if (lookahead == 'r') ADVANCE(1289);
      END_STATE();
    case 1142:
      if (lookahead == 'o') ADVANCE(1290);
      END_STATE();
    case 1143:
      if (lookahead == 't') ADVANCE(1291);
      END_STATE();
    case 1144:
      if (lookahead == 'e') ADVANCE(1292);
      END_STATE();
    case 1145:
      if (lookahead == 'D') ADVANCE(1293);
      END_STATE();
    case 1146:
      if (lookahead == 'D') ADVANCE(1294);
      END_STATE();
    case 1147:
      if (lookahead == 'D') ADVANCE(1295);
      END_STATE();
    case 1148:
      if (lookahead == 'u') ADVANCE(1296);
      END_STATE();
    case 1149:
      if (lookahead == 'S') ADVANCE(1297);
      END_STATE();
    case 1150:
      if (lookahead == 'a') ADVANCE(1298);
      END_STATE();
    case 1151:
      ACCEPT_TOKEN(anon_sym_abstract);
      END_STATE();
    case 1152:
      if (lookahead == 'm') ADVANCE(1299);
      END_STATE();
    case 1153:
      if (lookahead == 'i') ADVANCE(1300);
      END_STATE();
    case 1154:
      if (lookahead == 'e') ADVANCE(1301);
      END_STATE();
    case 1155:
      if (lookahead == 'a') ADVANCE(1302);
      END_STATE();
    case 1156:
      ACCEPT_TOKEN(anon_sym_centroid);
      END_STATE();
    case 1157:
      ACCEPT_TOKEN(anon_sym_co_await);
      END_STATE();
    case 1158:
      if (lookahead == 'n') ADVANCE(1303);
      END_STATE();
    case 1159:
      ACCEPT_TOKEN(anon_sym_co_yield);
      END_STATE();
    case 1160:
      ACCEPT_TOKEN(anon_sym_coherent);
      END_STATE();
    case 1161:
      if (lookahead == 'a') ADVANCE(1304);
      END_STATE();
    case 1162:
      if (lookahead == 'f') ADVANCE(1305);
      END_STATE();
    case 1163:
      if (lookahead == 's') ADVANCE(1306);
      END_STATE();
    case 1164:
      if (lookahead == 'l') ADVANCE(1307);
      END_STATE();
    case 1165:
      if (lookahead == 'r') ADVANCE(1308);
      END_STATE();
    case 1166:
      if (lookahead == 't') ADVANCE(1309);
      END_STATE();
    case 1167:
      ACCEPT_TOKEN(sym_continue);
      END_STATE();
    case 1168:
      if (lookahead == 'n') ADVANCE(1310);
      END_STATE();
    case 1169:
      ACCEPT_TOKEN(anon_sym_debugger);
      END_STATE();
    case 1170:
      ACCEPT_TOKEN(anon_sym_decltype);
      END_STATE();
    case 1171:
      if (lookahead == 'o') ADVANCE(1311);
      END_STATE();
    case 1172:
      if (lookahead == 'c') ADVANCE(1312);
      END_STATE();
    case 1173:
      ACCEPT_TOKEN(anon_sym_explicit);
      END_STATE();
    case 1174:
      ACCEPT_TOKEN(anon_sym_external);
      END_STATE();
    case 1175:
      if (lookahead == 'u') ADVANCE(1313);
      END_STATE();
    case 1176:
      ACCEPT_TOKEN(sym_function);
      END_STATE();
    case 1177:
      if (lookahead == 'r') ADVANCE(1314);
      END_STATE();
    case 1178:
      ACCEPT_TOKEN(anon_sym_iimage1D);
      if (lookahead == 'A') ADVANCE(1315);
      END_STATE();
    case 1179:
      ACCEPT_TOKEN(anon_sym_iimage2D);
      if (lookahead == 'A') ADVANCE(1316);
      if (lookahead == 'M') ADVANCE(1317);
      if (lookahead == 'R') ADVANCE(1318);
      END_STATE();
    case 1180:
      ACCEPT_TOKEN(anon_sym_iimage3D);
      END_STATE();
    case 1181:
      if (lookahead == 'f') ADVANCE(1319);
      END_STATE();
    case 1182:
      if (lookahead == 'b') ADVANCE(1320);
      END_STATE();
    case 1183:
      if (lookahead == 'r') ADVANCE(1321);
      END_STATE();
    case 1184:
      if (lookahead == 'r') ADVANCE(1322);
      END_STATE();
    case 1185:
      if (lookahead == 'S') ADVANCE(1323);
      END_STATE();
    case 1186:
      if (lookahead == 'e') ADVANCE(1324);
      END_STATE();
    case 1187:
      if (lookahead == 'f') ADVANCE(1325);
      END_STATE();
    case 1188:
      if (lookahead == 'e') ADVANCE(1326);
      END_STATE();
    case 1189:
      if (lookahead == 't') ADVANCE(1327);
      END_STATE();
    case 1190:
      if (lookahead == 'o') ADVANCE(1328);
      END_STATE();
    case 1191:
      if (lookahead == 'e') ADVANCE(1329);
      END_STATE();
    case 1192:
      if (lookahead == 't') ADVANCE(1330);
      END_STATE();
    case 1193:
      if (lookahead == '1') ADVANCE(1331);
      if (lookahead == '2') ADVANCE(1332);
      if (lookahead == '3') ADVANCE(1333);
      if (lookahead == 'B') ADVANCE(1334);
      if (lookahead == 'C') ADVANCE(1335);
      END_STATE();
    case 1194:
      if (lookahead == 'I') ADVANCE(1336);
      END_STATE();
    case 1195:
      if (lookahead == '1') ADVANCE(1337);
      if (lookahead == '2') ADVANCE(1338);
      if (lookahead == '3') ADVANCE(1339);
      if (lookahead == 'B') ADVANCE(1340);
      if (lookahead == 'C') ADVANCE(1341);
      END_STATE();
    case 1196:
      if (lookahead == 'l') ADVANCE(1342);
      END_STATE();
    case 1197:
      if (lookahead == 'e') ADVANCE(1343);
      END_STATE();
    case 1198:
      ACCEPT_TOKEN(anon_sym_noexcept);
      END_STATE();
    case 1199:
      ACCEPT_TOKEN(anon_sym_noinline);
      END_STATE();
    case 1200:
      if (lookahead == 'o') ADVANCE(1344);
      END_STATE();
    case 1201:
      if (lookahead == 'c') ADVANCE(1345);
      END_STATE();
    case 1202:
      ACCEPT_TOKEN(anon_sym_operator);
      END_STATE();
    case 1203:
      ACCEPT_TOKEN(sym_override);
      END_STATE();
    case 1204:
      if (lookahead == 'e') ADVANCE(1346);
      END_STATE();
    case 1205:
      if (lookahead == 'n') ADVANCE(1347);
      END_STATE();
    case 1206:
      if (lookahead == 'g') ADVANCE(1348);
      END_STATE();
    case 1207:
      if (lookahead == 'n') ADVANCE(1349);
      END_STATE();
    case 1208:
      ACCEPT_TOKEN(anon_sym_premerge);
      END_STATE();
    case 1209:
      if (lookahead == 'd') ADVANCE(1350);
      END_STATE();
    case 1210:
      ACCEPT_TOKEN(anon_sym_r32float);
      END_STATE();
    case 1211:
      if (lookahead == 't') ADVANCE(1351);
      END_STATE();
    case 1212:
      ACCEPT_TOKEN(anon_sym_readonly);
      END_STATE();
    case 1213:
      if (lookahead == 's') ADVANCE(1352);
      END_STATE();
    case 1214:
      ACCEPT_TOKEN(anon_sym_register);
      END_STATE();
    case 1215:
      if (lookahead == 'r') ADVANCE(1353);
      END_STATE();
    case 1216:
      ACCEPT_TOKEN(anon_sym_requires);
      END_STATE();
    case 1217:
      ACCEPT_TOKEN(anon_sym_resource);
      END_STATE();
    case 1218:
      ACCEPT_TOKEN(anon_sym_restrict);
      END_STATE();
    case 1219:
      if (lookahead == 't') ADVANCE(1354);
      END_STATE();
    case 1220:
      ACCEPT_TOKEN(anon_sym_rg32sint);
      END_STATE();
    case 1221:
      ACCEPT_TOKEN(anon_sym_rg32uint);
      END_STATE();
    case 1222:
      if (lookahead == 'o') ADVANCE(1355);
      END_STATE();
    case 1223:
      if (lookahead == 'n') ADVANCE(1356);
      END_STATE();
    case 1224:
      if (lookahead == 'n') ADVANCE(1357);
      END_STATE();
    case 1225:
      if (lookahead == 'o') ADVANCE(1358);
      END_STATE();
    case 1226:
      if (lookahead == 'n') ADVANCE(1359);
      END_STATE();
    case 1227:
      if (lookahead == 'n') ADVANCE(1360);
      END_STATE();
    case 1228:
      if (lookahead == 't') ADVANCE(1361);
      END_STATE();
    case 1229:
      if (lookahead == 'r') ADVANCE(1362);
      END_STATE();
    case 1230:
      if (lookahead == 't') ADVANCE(1363);
      END_STATE();
    case 1231:
      if (lookahead == 'r') ADVANCE(1364);
      END_STATE();
    case 1232:
      if (lookahead == 'r') ADVANCE(1365);
      END_STATE();
    case 1233:
      if (lookahead == 'D') ADVANCE(1366);
      END_STATE();
    case 1234:
      if (lookahead == 'D') ADVANCE(1367);
      END_STATE();
    case 1235:
      if (lookahead == 'D') ADVANCE(1368);
      END_STATE();
    case 1236:
      if (lookahead == 'u') ADVANCE(1369);
      END_STATE();
    case 1237:
      if (lookahead == 'u') ADVANCE(1370);
      END_STATE();
    case 1238:
      if (lookahead == 'h') ADVANCE(1371);
      END_STATE();
    case 1239:
      if (lookahead == 'c') ADVANCE(1372);
      END_STATE();
    case 1240:
      if (lookahead == 'c') ADVANCE(1373);
      END_STATE();
    case 1241:
      if (lookahead == 's') ADVANCE(1374);
      END_STATE();
    case 1242:
      if (lookahead == 's') ADVANCE(1375);
      END_STATE();
    case 1243:
      if (lookahead == 'a') ADVANCE(1376);
      END_STATE();
    case 1244:
      if (lookahead == 'n') ADVANCE(1377);
      END_STATE();
    case 1245:
      if (lookahead == 'n') ADVANCE(1378);
      END_STATE();
    case 1246:
      if (lookahead == 'e') ADVANCE(1379);
      END_STATE();
    case 1247:
      ACCEPT_TOKEN(anon_sym_template);
      END_STATE();
    case 1248:
      if (lookahead == 'D') ADVANCE(1380);
      END_STATE();
    case 1249:
      if (lookahead == 'D') ADVANCE(1381);
      END_STATE();
    case 1250:
      if (lookahead == 'D') ADVANCE(1382);
      END_STATE();
    case 1251:
      if (lookahead == 'u') ADVANCE(1383);
      END_STATE();
    case 1252:
      if (lookahead == 'u') ADVANCE(1384);
      END_STATE();
    case 1253:
      if (lookahead == '1') ADVANCE(1385);
      if (lookahead == '2') ADVANCE(1386);
      if (lookahead == '3') ADVANCE(1387);
      if (lookahead == 'c') ADVANCE(1388);
      if (lookahead == 'd') ADVANCE(1389);
      if (lookahead == 'm') ADVANCE(1390);
      if (lookahead == 's') ADVANCE(1391);
      END_STATE();
    case 1254:
      if (lookahead == 'o') ADVANCE(1392);
      END_STATE();
    case 1255:
      ACCEPT_TOKEN(anon_sym_triangle);
      if (lookahead == 'a') ADVANCE(1393);
      END_STATE();
    case 1256:
      ACCEPT_TOKEN(anon_sym_typename);
      END_STATE();
    case 1257:
      ACCEPT_TOKEN(anon_sym_uimage1D);
      if (lookahead == 'A') ADVANCE(1394);
      END_STATE();
    case 1258:
      ACCEPT_TOKEN(anon_sym_uimage2D);
      if (lookahead == 'A') ADVANCE(1395);
      if (lookahead == 'M') ADVANCE(1396);
      if (lookahead == 'R') ADVANCE(1397);
      END_STATE();
    case 1259:
      ACCEPT_TOKEN(anon_sym_uimage3D);
      END_STATE();
    case 1260:
      if (lookahead == 'f') ADVANCE(1398);
      END_STATE();
    case 1261:
      if (lookahead == 'b') ADVANCE(1399);
      END_STATE();
    case 1262:
      ACCEPT_TOKEN(anon_sym_unsigned);
      END_STATE();
    case 1263:
      if (lookahead == '1') ADVANCE(1400);
      if (lookahead == '2') ADVANCE(1401);
      if (lookahead == '3') ADVANCE(1402);
      if (lookahead == 'B') ADVANCE(1403);
      if (lookahead == 'C') ADVANCE(1404);
      END_STATE();
    case 1264:
      if (lookahead == 'I') ADVANCE(1405);
      END_STATE();
    case 1265:
      if (lookahead == '1') ADVANCE(1406);
      if (lookahead == '2') ADVANCE(1407);
      if (lookahead == '3') ADVANCE(1408);
      if (lookahead == 'B') ADVANCE(1409);
      if (lookahead == 'C') ADVANCE(1410);
      END_STATE();
    case 1266:
      if (lookahead == 'a') ADVANCE(1411);
      END_STATE();
    case 1267:
      ACCEPT_TOKEN(anon_sym_volatile);
      END_STATE();
    case 1268:
      if (lookahead == 'p') ADVANCE(1412);
      END_STATE();
    case 1269:
      if (lookahead == 'y') ADVANCE(1413);
      END_STATE();
    case 1270:
      if (lookahead == 'u') ADVANCE(1414);
      END_STATE();
    case 1271:
      if (lookahead == 'e') ADVANCE(1415);
      END_STATE();
    case 1272:
      if (lookahead == 's') ADVANCE(1416);
      END_STATE();
    case 1273:
      if (lookahead == 'a') ADVANCE(1417);
      END_STATE();
    case 1274:
      if (lookahead == 'a') ADVANCE(1418);
      END_STATE();
    case 1275:
      if (lookahead == 'r') ADVANCE(1419);
      END_STATE();
    case 1276:
      if (lookahead == 'c') ADVANCE(1420);
      END_STATE();
    case 1277:
      if (lookahead == 'd') ADVANCE(1421);
      END_STATE();
    case 1278:
      if (lookahead == 'h') ADVANCE(1422);
      END_STATE();
    case 1279:
      if (lookahead == 'r') ADVANCE(1423);
      END_STATE();
    case 1280:
      if (lookahead == 'h') ADVANCE(1424);
      END_STATE();
    case 1281:
      if (lookahead == 'm') ADVANCE(1425);
      END_STATE();
    case 1282:
      if (lookahead == 'c') ADVANCE(1426);
      END_STATE();
    case 1283:
      if (lookahead == 'e') ADVANCE(1427);
      END_STATE();
    case 1284:
      if (lookahead == 'a') ADVANCE(1428);
      END_STATE();
    case 1285:
      if (lookahead == 'r') ADVANCE(1429);
      END_STATE();
    case 1286:
      if (lookahead == 'r') ADVANCE(1430);
      END_STATE();
    case 1287:
      if (lookahead == '1') ADVANCE(1431);
      if (lookahead == '2') ADVANCE(1432);
      if (lookahead == '3') ADVANCE(1433);
      END_STATE();
    case 1288:
      if (lookahead == 'r') ADVANCE(1434);
      END_STATE();
    case 1289:
      if (lookahead == 'g') ADVANCE(1435);
      END_STATE();
    case 1290:
      if (lookahead == 'm') ADVANCE(1436);
      END_STATE();
    case 1291:
      if (lookahead == 'a') ADVANCE(1437);
      END_STATE();
    case 1292:
      if (lookahead == 'd') ADVANCE(1438);
      END_STATE();
    case 1293:
      ACCEPT_TOKEN(anon_sym_Texture1D);
      if (lookahead == 'A') ADVANCE(1439);
      END_STATE();
    case 1294:
      ACCEPT_TOKEN(anon_sym_Texture2D);
      if (lookahead == 'A') ADVANCE(1440);
      if (lookahead == 'M') ADVANCE(1441);
      END_STATE();
    case 1295:
      ACCEPT_TOKEN(anon_sym_Texture3D);
      END_STATE();
    case 1296:
      if (lookahead == 'b') ADVANCE(1442);
      END_STATE();
    case 1297:
      if (lookahead == 't') ADVANCE(1443);
      END_STATE();
    case 1298:
      if (lookahead == 'd') ADVANCE(1444);
      END_STATE();
    case 1299:
      if (lookahead == 'e') ADVANCE(1445);
      END_STATE();
    case 1300:
      if (lookahead == 'n') ADVANCE(1446);
      END_STATE();
    case 1301:
      ACCEPT_TOKEN(anon_sym_attribute);
      END_STATE();
    case 1302:
      if (lookahead == 'r') ADVANCE(1447);
      END_STATE();
    case 1303:
      ACCEPT_TOKEN(anon_sym_co_return);
      END_STATE();
    case 1304:
      if (lookahead == 'j') ADVANCE(1448);
      END_STATE();
    case 1305:
      if (lookahead == 'r') ADVANCE(1449);
      END_STATE();
    case 1306:
      if (lookahead == 't') ADVANCE(1450);
      END_STATE();
    case 1307:
      ACCEPT_TOKEN(anon_sym_consteval);
      END_STATE();
    case 1308:
      ACCEPT_TOKEN(anon_sym_constexpr);
      END_STATE();
    case 1309:
      ACCEPT_TOKEN(anon_sym_constinit);
      END_STATE();
    case 1310:
      if (lookahead == 'g') ADVANCE(1451);
      END_STATE();
    case 1311:
      if (lookahead == '_') ADVANCE(1452);
      END_STATE();
    case 1312:
      if (lookahead == 'a') ADVANCE(1453);
      END_STATE();
    case 1313:
      if (lookahead == 'g') ADVANCE(1454);
      END_STATE();
    case 1314:
      if (lookahead == 'e') ADVANCE(1455);
      END_STATE();
    case 1315:
      if (lookahead == 'r') ADVANCE(1456);
      END_STATE();
    case 1316:
      if (lookahead == 'r') ADVANCE(1457);
      END_STATE();
    case 1317:
      if (lookahead == 'S') ADVANCE(1458);
      END_STATE();
    case 1318:
      if (lookahead == 'e') ADVANCE(1459);
      END_STATE();
    case 1319:
      if (lookahead == 'f') ADVANCE(1460);
      END_STATE();
    case 1320:
      if (lookahead == 'e') ADVANCE(1461);
      END_STATE();
    case 1321:
      if (lookahead == 'r') ADVANCE(1462);
      END_STATE();
    case 1322:
      if (lookahead == 'r') ADVANCE(1463);
      END_STATE();
    case 1323:
      ACCEPT_TOKEN(anon_sym_image2DMS);
      if (lookahead == 'A') ADVANCE(1464);
      END_STATE();
    case 1324:
      if (lookahead == 'c') ADVANCE(1465);
      END_STATE();
    case 1325:
      if (lookahead == 'e') ADVANCE(1466);
      END_STATE();
    case 1326:
      ACCEPT_TOKEN(anon_sym_imageCube);
      if (lookahead == 'A') ADVANCE(1467);
      END_STATE();
    case 1327:
      if (lookahead == 's') ADVANCE(1468);
      END_STATE();
    case 1328:
      if (lookahead == 'f') ADVANCE(1469);
      END_STATE();
    case 1329:
      ACCEPT_TOKEN(anon_sym_interface);
      END_STATE();
    case 1330:
      ACCEPT_TOKEN(anon_sym_invariant);
      END_STATE();
    case 1331:
      if (lookahead == 'D') ADVANCE(1470);
      END_STATE();
    case 1332:
      if (lookahead == 'D') ADVANCE(1471);
      END_STATE();
    case 1333:
      if (lookahead == 'D') ADVANCE(1472);
      END_STATE();
    case 1334:
      if (lookahead == 'u') ADVANCE(1473);
      END_STATE();
    case 1335:
      if (lookahead == 'u') ADVANCE(1474);
      END_STATE();
    case 1336:
      if (lookahead == 'n') ADVANCE(1475);
      END_STATE();
    case 1337:
      if (lookahead == 'D') ADVANCE(1476);
      END_STATE();
    case 1338:
      if (lookahead == 'D') ADVANCE(1477);
      END_STATE();
    case 1339:
      if (lookahead == 'D') ADVANCE(1478);
      END_STATE();
    case 1340:
      if (lookahead == 'u') ADVANCE(1479);
      END_STATE();
    case 1341:
      if (lookahead == 'u') ADVANCE(1480);
      END_STATE();
    case 1342:
      if (lookahead == 'e') ADVANCE(1481);
      END_STATE();
    case 1343:
      ACCEPT_TOKEN(anon_sym_namespace);
      END_STATE();
    case 1344:
      if (lookahead == 'l') ADVANCE(1482);
      END_STATE();
    case 1345:
      if (lookahead == 't') ADVANCE(1483);
      END_STATE();
    case 1346:
      if (lookahead == 't') ADVANCE(1484);
      END_STATE();
    case 1347:
      ACCEPT_TOKEN(anon_sym_partition);
      END_STATE();
    case 1348:
      if (lookahead == 'm') ADVANCE(1485);
      END_STATE();
    case 1349:
      ACCEPT_TOKEN(anon_sym_precision);
      END_STATE();
    case 1350:
      ACCEPT_TOKEN(anon_sym_protected);
      END_STATE();
    case 1351:
      if (lookahead == 'e') ADVANCE(1486);
      END_STATE();
    case 1352:
      if (lookahead == 's') ADVANCE(1487);
      END_STATE();
    case 1353:
      if (lookahead == 'e') ADVANCE(1488);
      END_STATE();
    case 1354:
      ACCEPT_TOKEN(anon_sym_rg32float);
      END_STATE();
    case 1355:
      if (lookahead == 'a') ADVANCE(1489);
      END_STATE();
    case 1356:
      if (lookahead == 't') ADVANCE(1490);
      END_STATE();
    case 1357:
      if (lookahead == 't') ADVANCE(1491);
      END_STATE();
    case 1358:
      if (lookahead == 'a') ADVANCE(1492);
      END_STATE();
    case 1359:
      if (lookahead == 't') ADVANCE(1493);
      END_STATE();
    case 1360:
      if (lookahead == 't') ADVANCE(1494);
      END_STATE();
    case 1361:
      ACCEPT_TOKEN(anon_sym_rgba8sint);
      END_STATE();
    case 1362:
      if (lookahead == 'm') ADVANCE(1495);
      END_STATE();
    case 1363:
      ACCEPT_TOKEN(anon_sym_rgba8uint);
      END_STATE();
    case 1364:
      if (lookahead == 'm') ADVANCE(1496);
      END_STATE();
    case 1365:
      ACCEPT_TOKEN(anon_sym_row_major);
      END_STATE();
    case 1366:
      ACCEPT_TOKEN(anon_sym_sampler1D);
      if (lookahead == 'A') ADVANCE(1497);
      if (lookahead == 'S') ADVANCE(1498);
      END_STATE();
    case 1367:
      ACCEPT_TOKEN(anon_sym_sampler2D);
      if (lookahead == 'A') ADVANCE(1499);
      if (lookahead == 'M') ADVANCE(1500);
      if (lookahead == 'R') ADVANCE(1501);
      if (lookahead == 'S') ADVANCE(1502);
      END_STATE();
    case 1368:
      ACCEPT_TOKEN(anon_sym_sampler3D);
      if (lookahead == 'R') ADVANCE(1503);
      END_STATE();
    case 1369:
      if (lookahead == 'f') ADVANCE(1504);
      END_STATE();
    case 1370:
      if (lookahead == 'b') ADVANCE(1505);
      END_STATE();
    case 1371:
      if (lookahead == 'a') ADVANCE(1506);
      END_STATE();
    case 1372:
      if (lookahead == 'o') ADVANCE(1507);
      END_STATE();
    case 1373:
      if (lookahead == 'k') ADVANCE(1508);
      END_STATE();
    case 1374:
      if (lookahead == 'e') ADVANCE(1509);
      END_STATE();
    case 1375:
      if (lookahead == 's') ADVANCE(1510);
      END_STATE();
    case 1376:
      if (lookahead == 's') ADVANCE(1511);
      END_STATE();
    case 1377:
      if (lookahead == 'p') ADVANCE(1512);
      END_STATE();
    case 1378:
      if (lookahead == 'e') ADVANCE(1513);
      END_STATE();
    case 1379:
      ACCEPT_TOKEN(anon_sym_technique);
      if (lookahead == '1') ADVANCE(1514);
      END_STATE();
    case 1380:
      ACCEPT_TOKEN(anon_sym_texture1D);
      if (lookahead == 'A') ADVANCE(1515);
      END_STATE();
    case 1381:
      ACCEPT_TOKEN(anon_sym_texture2D);
      if (lookahead == 'A') ADVANCE(1516);
      if (lookahead == 'M') ADVANCE(1517);
      if (lookahead == 'R') ADVANCE(1518);
      END_STATE();
    case 1382:
      ACCEPT_TOKEN(anon_sym_texture3D);
      END_STATE();
    case 1383:
      if (lookahead == 'f') ADVANCE(1519);
      END_STATE();
    case 1384:
      if (lookahead == 'b') ADVANCE(1520);
      END_STATE();
    case 1385:
      if (lookahead == 'd') ADVANCE(1521);
      END_STATE();
    case 1386:
      if (lookahead == 'd') ADVANCE(1522);
      END_STATE();
    case 1387:
      if (lookahead == 'd') ADVANCE(1523);
      END_STATE();
    case 1388:
      if (lookahead == 'u') ADVANCE(1524);
      END_STATE();
    case 1389:
      if (lookahead == 'e') ADVANCE(1525);
      END_STATE();
    case 1390:
      if (lookahead == 'u') ADVANCE(1526);
      END_STATE();
    case 1391:
      if (lookahead == 't') ADVANCE(1527);
      END_STATE();
    case 1392:
      if (lookahead == 'c') ADVANCE(1528);
      END_STATE();
    case 1393:
      if (lookahead == 'd') ADVANCE(1529);
      END_STATE();
    case 1394:
      if (lookahead == 'r') ADVANCE(1530);
      END_STATE();
    case 1395:
      if (lookahead == 'r') ADVANCE(1531);
      END_STATE();
    case 1396:
      if (lookahead == 'S') ADVANCE(1532);
      END_STATE();
    case 1397:
      if (lookahead == 'e') ADVANCE(1533);
      END_STATE();
    case 1398:
      if (lookahead == 'f') ADVANCE(1534);
      END_STATE();
    case 1399:
      if (lookahead == 'e') ADVANCE(1535);
      END_STATE();
    case 1400:
      if (lookahead == 'D') ADVANCE(1536);
      END_STATE();
    case 1401:
      if (lookahead == 'D') ADVANCE(1537);
      END_STATE();
    case 1402:
      if (lookahead == 'D') ADVANCE(1538);
      END_STATE();
    case 1403:
      if (lookahead == 'u') ADVANCE(1539);
      END_STATE();
    case 1404:
      if (lookahead == 'u') ADVANCE(1540);
      END_STATE();
    case 1405:
      if (lookahead == 'n') ADVANCE(1541);
      END_STATE();
    case 1406:
      if (lookahead == 'D') ADVANCE(1542);
      END_STATE();
    case 1407:
      if (lookahead == 'D') ADVANCE(1543);
      END_STATE();
    case 1408:
      if (lookahead == 'D') ADVANCE(1544);
      END_STATE();
    case 1409:
      if (lookahead == 'u') ADVANCE(1545);
      END_STATE();
    case 1410:
      if (lookahead == 'u') ADVANCE(1546);
      END_STATE();
    case 1411:
      if (lookahead == 'g') ADVANCE(1547);
      END_STATE();
    case 1412:
      ACCEPT_TOKEN(sym_workgroup);
      END_STATE();
    case 1413:
      ACCEPT_TOKEN(anon_sym_writeonly);
      END_STATE();
    case 1414:
      if (lookahead == 'c') ADVANCE(1548);
      END_STATE();
    case 1415:
      ACCEPT_TOKEN(anon_sym_BlendState);
      END_STATE();
    case 1416:
      if (lookahead == 's') ADVANCE(1549);
      END_STATE();
    case 1417:
      if (lookahead == 'd') ADVANCE(1550);
      END_STATE();
    case 1418:
      if (lookahead == 'd') ADVANCE(1551);
      END_STATE();
    case 1419:
      if (lookahead == 'u') ADVANCE(1552);
      END_STATE();
    case 1420:
      if (lookahead == 'i') ADVANCE(1553);
      END_STATE();
    case 1421:
      if (lookahead == 'e') ADVANCE(1554);
      END_STATE();
    case 1422:
      if (lookahead == 'a') ADVANCE(1555);
      END_STATE();
    case 1423:
      ACCEPT_TOKEN(anon_sym_Hullshader);
      END_STATE();
    case 1424:
      ACCEPT_TOKEN(anon_sym_InputPatch);
      END_STATE();
    case 1425:
      ACCEPT_TOKEN(anon_sym_LineStream);
      END_STATE();
    case 1426:
      if (lookahead == 'h') ADVANCE(1556);
      END_STATE();
    case 1427:
      if (lookahead == 'r') ADVANCE(1557);
      END_STATE();
    case 1428:
      if (lookahead == 'm') ADVANCE(1558);
      END_STATE();
    case 1429:
      if (lookahead == 'e') ADVANCE(1559);
      END_STATE();
    case 1430:
      if (lookahead == 'e') ADVANCE(1560);
      END_STATE();
    case 1431:
      if (lookahead == 'D') ADVANCE(1561);
      END_STATE();
    case 1432:
      if (lookahead == 'D') ADVANCE(1562);
      END_STATE();
    case 1433:
      if (lookahead == 'D') ADVANCE(1563);
      END_STATE();
    case 1434:
      if (lookahead == 'S') ADVANCE(1564);
      END_STATE();
    case 1435:
      if (lookahead == 'e') ADVANCE(1565);
      END_STATE();
    case 1436:
      if (lookahead == 'p') ADVANCE(1566);
      END_STATE();
    case 1437:
      if (lookahead == 't') ADVANCE(1567);
      END_STATE();
    case 1438:
      if (lookahead == 'B') ADVANCE(1568);
      END_STATE();
    case 1439:
      if (lookahead == 'r') ADVANCE(1569);
      END_STATE();
    case 1440:
      if (lookahead == 'r') ADVANCE(1570);
      END_STATE();
    case 1441:
      if (lookahead == 'S') ADVANCE(1571);
      END_STATE();
    case 1442:
      if (lookahead == 'e') ADVANCE(1572);
      END_STATE();
    case 1443:
      if (lookahead == 'r') ADVANCE(1573);
      END_STATE();
    case 1444:
      if (lookahead == 'e') ADVANCE(1574);
      END_STATE();
    case 1445:
      if (lookahead == 'n') ADVANCE(1575);
      END_STATE();
    case 1446:
      if (lookahead == 't') ADVANCE(1576);
      END_STATE();
    case 1447:
      if (lookahead == 'r') ADVANCE(1577);
      END_STATE();
    case 1448:
      if (lookahead == 'o') ADVANCE(1578);
      END_STATE();
    case 1449:
      if (lookahead == 'a') ADVANCE(1579);
      END_STATE();
    case 1450:
      ACCEPT_TOKEN(anon_sym_const_cast);
      END_STATE();
    case 1451:
      ACCEPT_TOKEN(sym_continuing);
      END_STATE();
    case 1452:
      if (lookahead == 'h') ADVANCE(1580);
      END_STATE();
    case 1453:
      if (lookahead == 's') ADVANCE(1581);
      END_STATE();
    case 1454:
      if (lookahead == 'h') ADVANCE(1582);
      END_STATE();
    case 1455:
      if (lookahead == 'd') ADVANCE(1583);
      END_STATE();
    case 1456:
      if (lookahead == 'r') ADVANCE(1584);
      END_STATE();
    case 1457:
      if (lookahead == 'r') ADVANCE(1585);
      END_STATE();
    case 1458:
      ACCEPT_TOKEN(anon_sym_iimage2DMS);
      if (lookahead == 'A') ADVANCE(1586);
      END_STATE();
    case 1459:
      if (lookahead == 'c') ADVANCE(1587);
      END_STATE();
    case 1460:
      if (lookahead == 'e') ADVANCE(1588);
      END_STATE();
    case 1461:
      ACCEPT_TOKEN(anon_sym_iimageCube);
      if (lookahead == 'A') ADVANCE(1589);
      END_STATE();
    case 1462:
      if (lookahead == 'a') ADVANCE(1590);
      END_STATE();
    case 1463:
      if (lookahead == 'a') ADVANCE(1591);
      END_STATE();
    case 1464:
      if (lookahead == 'r') ADVANCE(1592);
      END_STATE();
    case 1465:
      if (lookahead == 't') ADVANCE(1593);
      END_STATE();
    case 1466:
      if (lookahead == 'r') ADVANCE(1594);
      END_STATE();
    case 1467:
      if (lookahead == 'r') ADVANCE(1595);
      END_STATE();
    case 1468:
      ACCEPT_TOKEN(anon_sym_implements);
      END_STATE();
    case 1469:
      ACCEPT_TOKEN(anon_sym_instanceof);
      END_STATE();
    case 1470:
      ACCEPT_TOKEN(anon_sym_isampler1D);
      if (lookahead == 'A') ADVANCE(1596);
      END_STATE();
    case 1471:
      ACCEPT_TOKEN(anon_sym_isampler2D);
      if (lookahead == 'A') ADVANCE(1597);
      if (lookahead == 'M') ADVANCE(1598);
      if (lookahead == 'R') ADVANCE(1599);
      END_STATE();
    case 1472:
      ACCEPT_TOKEN(anon_sym_isampler3D);
      END_STATE();
    case 1473:
      if (lookahead == 'f') ADVANCE(1600);
      END_STATE();
    case 1474:
      if (lookahead == 'b') ADVANCE(1601);
      END_STATE();
    case 1475:
      if (lookahead == 'p') ADVANCE(1602);
      END_STATE();
    case 1476:
      ACCEPT_TOKEN(anon_sym_itexture1D);
      if (lookahead == 'A') ADVANCE(1603);
      END_STATE();
    case 1477:
      ACCEPT_TOKEN(anon_sym_itexture2D);
      if (lookahead == 'A') ADVANCE(1604);
      if (lookahead == 'M') ADVANCE(1605);
      if (lookahead == 'R') ADVANCE(1606);
      END_STATE();
    case 1478:
      ACCEPT_TOKEN(anon_sym_itexture3D);
      END_STATE();
    case 1479:
      if (lookahead == 'f') ADVANCE(1607);
      END_STATE();
    case 1480:
      if (lookahead == 'b') ADVANCE(1608);
      END_STATE();
    case 1481:
      if (lookahead == 's') ADVANCE(1609);
      END_STATE();
    case 1482:
      if (lookahead == 'a') ADVANCE(1610);
      END_STATE();
    case 1483:
      if (lookahead == 'i') ADVANCE(1611);
      END_STATE();
    case 1484:
      ACCEPT_TOKEN(anon_sym_packoffset);
      END_STATE();
    case 1485:
      if (lookahead == 'e') ADVANCE(1612);
      END_STATE();
    case 1486:
      ACCEPT_TOKEN(anon_sym_read_write);
      END_STATE();
    case 1487:
      ACCEPT_TOKEN(anon_sym_regardless);
      END_STATE();
    case 1488:
      if (lookahead == 't') ADVANCE(1613);
      END_STATE();
    case 1489:
      if (lookahead == 't') ADVANCE(1614);
      END_STATE();
    case 1490:
      ACCEPT_TOKEN(anon_sym_rgba16sint);
      END_STATE();
    case 1491:
      ACCEPT_TOKEN(anon_sym_rgba16uint);
      END_STATE();
    case 1492:
      if (lookahead == 't') ADVANCE(1615);
      END_STATE();
    case 1493:
      ACCEPT_TOKEN(anon_sym_rgba32sint);
      END_STATE();
    case 1494:
      ACCEPT_TOKEN(anon_sym_rgba32uint);
      END_STATE();
    case 1495:
      ACCEPT_TOKEN(anon_sym_rgba8snorm);
      END_STATE();
    case 1496:
      ACCEPT_TOKEN(anon_sym_rgba8unorm);
      END_STATE();
    case 1497:
      if (lookahead == 'r') ADVANCE(1616);
      END_STATE();
    case 1498:
      if (lookahead == 'h') ADVANCE(1617);
      END_STATE();
    case 1499:
      if (lookahead == 'r') ADVANCE(1618);
      END_STATE();
    case 1500:
      if (lookahead == 'S') ADVANCE(1619);
      END_STATE();
    case 1501:
      if (lookahead == 'e') ADVANCE(1620);
      END_STATE();
    case 1502:
      if (lookahead == 'h') ADVANCE(1621);
      END_STATE();
    case 1503:
      if (lookahead == 'e') ADVANCE(1622);
      END_STATE();
    case 1504:
      if (lookahead == 'f') ADVANCE(1623);
      END_STATE();
    case 1505:
      if (lookahead == 'e') ADVANCE(1624);
      END_STATE();
    case 1506:
      if (lookahead == 'd') ADVANCE(1625);
      END_STATE();
    case 1507:
      if (lookahead == 'm') ADVANCE(1626);
      END_STATE();
    case 1508:
      ACCEPT_TOKEN(anon_sym_stateblock);
      if (lookahead == '_') ADVANCE(1627);
      END_STATE();
    case 1509:
      if (lookahead == 'r') ADVANCE(1628);
      END_STATE();
    case 1510:
      if (lookahead == 'e') ADVANCE(1629);
      END_STATE();
    case 1511:
      if (lookahead == 't') ADVANCE(1630);
      END_STATE();
    case 1512:
      if (lookahead == 'u') ADVANCE(1631);
      END_STATE();
    case 1513:
      ACCEPT_TOKEN(anon_sym_subroutine);
      END_STATE();
    case 1514:
      if (lookahead == '0') ADVANCE(1632);
      if (lookahead == '1') ADVANCE(1633);
      END_STATE();
    case 1515:
      if (lookahead == 'r') ADVANCE(1634);
      END_STATE();
    case 1516:
      if (lookahead == 'r') ADVANCE(1635);
      END_STATE();
    case 1517:
      if (lookahead == 'S') ADVANCE(1636);
      END_STATE();
    case 1518:
      if (lookahead == 'e') ADVANCE(1637);
      END_STATE();
    case 1519:
      if (lookahead == 'f') ADVANCE(1638);
      END_STATE();
    case 1520:
      if (lookahead == 'e') ADVANCE(1639);
      END_STATE();
    case 1521:
      ACCEPT_TOKEN(sym_texture_1d);
      END_STATE();
    case 1522:
      ACCEPT_TOKEN(sym_texture_2d);
      if (lookahead == '_') ADVANCE(1640);
      END_STATE();
    case 1523:
      ACCEPT_TOKEN(sym_texture_3d);
      END_STATE();
    case 1524:
      if (lookahead == 'b') ADVANCE(1641);
      END_STATE();
    case 1525:
      if (lookahead == 'p') ADVANCE(1642);
      END_STATE();
    case 1526:
      if (lookahead == 'l') ADVANCE(1643);
      END_STATE();
    case 1527:
      if (lookahead == 'o') ADVANCE(1644);
      END_STATE();
    case 1528:
      if (lookahead == 'a') ADVANCE(1645);
      END_STATE();
    case 1529:
      if (lookahead == 'j') ADVANCE(1646);
      END_STATE();
    case 1530:
      if (lookahead == 'r') ADVANCE(1647);
      END_STATE();
    case 1531:
      if (lookahead == 'r') ADVANCE(1648);
      END_STATE();
    case 1532:
      ACCEPT_TOKEN(anon_sym_uimage2DMS);
      if (lookahead == 'A') ADVANCE(1649);
      END_STATE();
    case 1533:
      if (lookahead == 'c') ADVANCE(1650);
      END_STATE();
    case 1534:
      if (lookahead == 'e') ADVANCE(1651);
      END_STATE();
    case 1535:
      ACCEPT_TOKEN(anon_sym_uimageCube);
      if (lookahead == 'A') ADVANCE(1652);
      END_STATE();
    case 1536:
      ACCEPT_TOKEN(anon_sym_usampler1D);
      if (lookahead == 'A') ADVANCE(1653);
      END_STATE();
    case 1537:
      ACCEPT_TOKEN(anon_sym_usampler2D);
      if (lookahead == 'A') ADVANCE(1654);
      if (lookahead == 'M') ADVANCE(1655);
      if (lookahead == 'R') ADVANCE(1656);
      END_STATE();
    case 1538:
      ACCEPT_TOKEN(anon_sym_usampler3D);
      END_STATE();
    case 1539:
      if (lookahead == 'f') ADVANCE(1657);
      END_STATE();
    case 1540:
      if (lookahead == 'b') ADVANCE(1658);
      END_STATE();
    case 1541:
      if (lookahead == 'p') ADVANCE(1659);
      END_STATE();
    case 1542:
      ACCEPT_TOKEN(anon_sym_utexture1D);
      if (lookahead == 'A') ADVANCE(1660);
      END_STATE();
    case 1543:
      ACCEPT_TOKEN(anon_sym_utexture2D);
      if (lookahead == 'A') ADVANCE(1661);
      if (lookahead == 'M') ADVANCE(1662);
      if (lookahead == 'R') ADVANCE(1663);
      END_STATE();
    case 1544:
      ACCEPT_TOKEN(anon_sym_utexture3D);
      END_STATE();
    case 1545:
      if (lookahead == 'f') ADVANCE(1664);
      END_STATE();
    case 1546:
      if (lookahead == 'b') ADVANCE(1665);
      END_STATE();
    case 1547:
      if (lookahead == 'm') ADVANCE(1666);
      END_STATE();
    case 1548:
      if (lookahead == 't') ADVANCE(1667);
      END_STATE();
    case 1549:
      if (lookahead == 'B') ADVANCE(1668);
      END_STATE();
    case 1550:
      if (lookahead == 'e') ADVANCE(1669);
      END_STATE();
    case 1551:
      if (lookahead == 'e') ADVANCE(1670);
      END_STATE();
    case 1552:
      if (lookahead == 'c') ADVANCE(1671);
      END_STATE();
    case 1553:
      if (lookahead == 'l') ADVANCE(1672);
      END_STATE();
    case 1554:
      if (lookahead == 'r') ADVANCE(1673);
      END_STATE();
    case 1555:
      if (lookahead == 'd') ADVANCE(1674);
      END_STATE();
    case 1556:
      ACCEPT_TOKEN(anon_sym_OutputPatch);
      END_STATE();
    case 1557:
      ACCEPT_TOKEN(anon_sym_PixelShader);
      END_STATE();
    case 1558:
      ACCEPT_TOKEN(anon_sym_PointStream);
      END_STATE();
    case 1559:
      if (lookahead == 's') ADVANCE(1675);
      END_STATE();
    case 1560:
      if (lookahead == 'd') ADVANCE(1676);
      END_STATE();
    case 1561:
      ACCEPT_TOKEN(anon_sym_RWTexture1D);
      if (lookahead == 'A') ADVANCE(1677);
      END_STATE();
    case 1562:
      ACCEPT_TOKEN(anon_sym_RWTexture2D);
      if (lookahead == 'A') ADVANCE(1678);
      END_STATE();
    case 1563:
      ACCEPT_TOKEN(anon_sym_RWTexture3D);
      END_STATE();
    case 1564:
      if (lookahead == 't') ADVANCE(1679);
      END_STATE();
    case 1565:
      if (lookahead == 't') ADVANCE(1680);
      END_STATE();
    case 1566:
      if (lookahead == 'a') ADVANCE(1681);
      END_STATE();
    case 1567:
      if (lookahead == 'e') ADVANCE(1682);
      END_STATE();
    case 1568:
      if (lookahead == 'u') ADVANCE(1683);
      END_STATE();
    case 1569:
      if (lookahead == 'r') ADVANCE(1684);
      END_STATE();
    case 1570:
      if (lookahead == 'r') ADVANCE(1685);
      END_STATE();
    case 1571:
      ACCEPT_TOKEN(anon_sym_Texture2DMS);
      if (lookahead == 'A') ADVANCE(1686);
      END_STATE();
    case 1572:
      ACCEPT_TOKEN(anon_sym_TextureCube);
      if (lookahead == 'A') ADVANCE(1687);
      END_STATE();
    case 1573:
      if (lookahead == 'e') ADVANCE(1688);
      END_STATE();
    case 1574:
      if (lookahead == 'r') ADVANCE(1689);
      END_STATE();
    case 1575:
      if (lookahead == 't') ADVANCE(1690);
      END_STATE();
    case 1576:
      ACCEPT_TOKEN(anon_sym_atomic_uint);
      END_STATE();
    case 1577:
      if (lookahead == 'a') ADVANCE(1691);
      END_STATE();
    case 1578:
      if (lookahead == 'r') ADVANCE(1692);
      END_STATE();
    case 1579:
      if (lookahead == 'g') ADVANCE(1693);
      END_STATE();
    case 1580:
      if (lookahead == 'e') ADVANCE(1694);
      END_STATE();
    case 1581:
      if (lookahead == 't') ADVANCE(1695);
      END_STATE();
    case 1582:
      ACCEPT_TOKEN(sym_fallthrough);
      END_STATE();
    case 1583:
      ACCEPT_TOKEN(anon_sym_groupshared);
      END_STATE();
    case 1584:
      if (lookahead == 'a') ADVANCE(1696);
      END_STATE();
    case 1585:
      if (lookahead == 'a') ADVANCE(1697);
      END_STATE();
    case 1586:
      if (lookahead == 'r') ADVANCE(1698);
      END_STATE();
    case 1587:
      if (lookahead == 't') ADVANCE(1699);
      END_STATE();
    case 1588:
      if (lookahead == 'r') ADVANCE(1700);
      END_STATE();
    case 1589:
      if (lookahead == 'r') ADVANCE(1701);
      END_STATE();
    case 1590:
      if (lookahead == 'y') ADVANCE(1702);
      END_STATE();
    case 1591:
      if (lookahead == 'y') ADVANCE(1703);
      END_STATE();
    case 1592:
      if (lookahead == 'r') ADVANCE(1704);
      END_STATE();
    case 1593:
      ACCEPT_TOKEN(anon_sym_image2DRect);
      END_STATE();
    case 1594:
      ACCEPT_TOKEN(anon_sym_imageBuffer);
      END_STATE();
    case 1595:
      if (lookahead == 'r') ADVANCE(1705);
      END_STATE();
    case 1596:
      if (lookahead == 'r') ADVANCE(1706);
      END_STATE();
    case 1597:
      if (lookahead == 'r') ADVANCE(1707);
      END_STATE();
    case 1598:
      if (lookahead == 'S') ADVANCE(1708);
      END_STATE();
    case 1599:
      if (lookahead == 'e') ADVANCE(1709);
      END_STATE();
    case 1600:
      if (lookahead == 'f') ADVANCE(1710);
      END_STATE();
    case 1601:
      if (lookahead == 'e') ADVANCE(1711);
      END_STATE();
    case 1602:
      if (lookahead == 'u') ADVANCE(1712);
      END_STATE();
    case 1603:
      if (lookahead == 'r') ADVANCE(1713);
      END_STATE();
    case 1604:
      if (lookahead == 'r') ADVANCE(1714);
      END_STATE();
    case 1605:
      if (lookahead == 'S') ADVANCE(1715);
      END_STATE();
    case 1606:
      if (lookahead == 'e') ADVANCE(1716);
      END_STATE();
    case 1607:
      if (lookahead == 'f') ADVANCE(1717);
      END_STATE();
    case 1608:
      if (lookahead == 'e') ADVANCE(1718);
      END_STATE();
    case 1609:
      ACCEPT_TOKEN(anon_sym_macro_rules);
      END_STATE();
    case 1610:
      if (lookahead == 't') ADVANCE(1719);
      END_STATE();
    case 1611:
      if (lookahead == 'v') ADVANCE(1720);
      END_STATE();
    case 1612:
      if (lookahead == 'n') ADVANCE(1721);
      END_STATE();
    case 1613:
      if (lookahead == '_') ADVANCE(1722);
      END_STATE();
    case 1614:
      ACCEPT_TOKEN(anon_sym_rgba16float);
      END_STATE();
    case 1615:
      ACCEPT_TOKEN(anon_sym_rgba32float);
      END_STATE();
    case 1616:
      if (lookahead == 'r') ADVANCE(1723);
      END_STATE();
    case 1617:
      if (lookahead == 'a') ADVANCE(1724);
      END_STATE();
    case 1618:
      if (lookahead == 'r') ADVANCE(1725);
      END_STATE();
    case 1619:
      ACCEPT_TOKEN(anon_sym_sampler2DMS);
      if (lookahead == 'A') ADVANCE(1726);
      END_STATE();
    case 1620:
      if (lookahead == 'c') ADVANCE(1727);
      END_STATE();
    case 1621:
      if (lookahead == 'a') ADVANCE(1728);
      END_STATE();
    case 1622:
      if (lookahead == 'c') ADVANCE(1729);
      END_STATE();
    case 1623:
      if (lookahead == 'e') ADVANCE(1730);
      END_STATE();
    case 1624:
      ACCEPT_TOKEN(anon_sym_samplerCube);
      if (lookahead == 'A') ADVANCE(1731);
      if (lookahead == 'S') ADVANCE(1732);
      END_STATE();
    case 1625:
      if (lookahead == 'o') ADVANCE(1733);
      END_STATE();
    case 1626:
      if (lookahead == 'p') ADVANCE(1734);
      END_STATE();
    case 1627:
      if (lookahead == 's') ADVANCE(1735);
      END_STATE();
    case 1628:
      if (lookahead == 't') ADVANCE(1736);
      END_STATE();
    case 1629:
      if (lookahead == 'r') ADVANCE(1737);
      END_STATE();
    case 1630:
      ACCEPT_TOKEN(anon_sym_static_cast);
      END_STATE();
    case 1631:
      if (lookahead == 't') ADVANCE(1738);
      END_STATE();
    case 1632:
      ACCEPT_TOKEN(anon_sym_technique10);
      END_STATE();
    case 1633:
      ACCEPT_TOKEN(anon_sym_technique11);
      END_STATE();
    case 1634:
      if (lookahead == 'r') ADVANCE(1739);
      END_STATE();
    case 1635:
      if (lookahead == 'r') ADVANCE(1740);
      END_STATE();
    case 1636:
      ACCEPT_TOKEN(anon_sym_texture2DMS);
      if (lookahead == 'A') ADVANCE(1741);
      END_STATE();
    case 1637:
      if (lookahead == 'c') ADVANCE(1742);
      END_STATE();
    case 1638:
      if (lookahead == 'e') ADVANCE(1743);
      END_STATE();
    case 1639:
      ACCEPT_TOKEN(anon_sym_textureCube);
      if (lookahead == 'A') ADVANCE(1744);
      END_STATE();
    case 1640:
      if (lookahead == 'a') ADVANCE(1745);
      END_STATE();
    case 1641:
      if (lookahead == 'e') ADVANCE(1746);
      END_STATE();
    case 1642:
      if (lookahead == 't') ADVANCE(1747);
      END_STATE();
    case 1643:
      if (lookahead == 't') ADVANCE(1748);
      END_STATE();
    case 1644:
      if (lookahead == 'r') ADVANCE(1749);
      END_STATE();
    case 1645:
      if (lookahead == 'l') ADVANCE(1750);
      END_STATE();
    case 1646:
      ACCEPT_TOKEN(anon_sym_triangleadj);
      END_STATE();
    case 1647:
      if (lookahead == 'a') ADVANCE(1751);
      END_STATE();
    case 1648:
      if (lookahead == 'a') ADVANCE(1752);
      END_STATE();
    case 1649:
      if (lookahead == 'r') ADVANCE(1753);
      END_STATE();
    case 1650:
      if (lookahead == 't') ADVANCE(1754);
      END_STATE();
    case 1651:
      if (lookahead == 'r') ADVANCE(1755);
      END_STATE();
    case 1652:
      if (lookahead == 'r') ADVANCE(1756);
      END_STATE();
    case 1653:
      if (lookahead == 'r') ADVANCE(1757);
      END_STATE();
    case 1654:
      if (lookahead == 'r') ADVANCE(1758);
      END_STATE();
    case 1655:
      if (lookahead == 'S') ADVANCE(1759);
      END_STATE();
    case 1656:
      if (lookahead == 'e') ADVANCE(1760);
      END_STATE();
    case 1657:
      if (lookahead == 'f') ADVANCE(1761);
      END_STATE();
    case 1658:
      if (lookahead == 'e') ADVANCE(1762);
      END_STATE();
    case 1659:
      if (lookahead == 'u') ADVANCE(1763);
      END_STATE();
    case 1660:
      if (lookahead == 'r') ADVANCE(1764);
      END_STATE();
    case 1661:
      if (lookahead == 'r') ADVANCE(1765);
      END_STATE();
    case 1662:
      if (lookahead == 'S') ADVANCE(1766);
      END_STATE();
    case 1663:
      if (lookahead == 'e') ADVANCE(1767);
      END_STATE();
    case 1664:
      if (lookahead == 'f') ADVANCE(1768);
      END_STATE();
    case 1665:
      if (lookahead == 'e') ADVANCE(1769);
      END_STATE();
    case 1666:
      if (lookahead == 'e') ADVANCE(1770);
      END_STATE();
    case 1667:
      if (lookahead == 'u') ADVANCE(1771);
      END_STATE();
    case 1668:
      if (lookahead == 'u') ADVANCE(1772);
      END_STATE();
    case 1669:
      if (lookahead == 'r') ADVANCE(1773);
      END_STATE();
    case 1670:
      if (lookahead == 'r') ADVANCE(1774);
      END_STATE();
    case 1671:
      if (lookahead == 't') ADVANCE(1775);
      END_STATE();
    case 1672:
      if (lookahead == 'S') ADVANCE(1776);
      if (lookahead == 'V') ADVANCE(1777);
      END_STATE();
    case 1673:
      ACCEPT_TOKEN(anon_sym_DomainShader);
      END_STATE();
    case 1674:
      if (lookahead == 'e') ADVANCE(1778);
      END_STATE();
    case 1675:
      if (lookahead == 's') ADVANCE(1779);
      END_STATE();
    case 1676:
      if (lookahead == 'B') ADVANCE(1780);
      END_STATE();
    case 1677:
      if (lookahead == 'r') ADVANCE(1781);
      END_STATE();
    case 1678:
      if (lookahead == 'r') ADVANCE(1782);
      END_STATE();
    case 1679:
      if (lookahead == 'a') ADVANCE(1783);
      END_STATE();
    case 1680:
      if (lookahead == 'V') ADVANCE(1784);
      END_STATE();
    case 1681:
      if (lookahead == 'r') ADVANCE(1785);
      END_STATE();
    case 1682:
      ACCEPT_TOKEN(anon_sym_SamplerState);
      END_STATE();
    case 1683:
      if (lookahead == 'f') ADVANCE(1786);
      END_STATE();
    case 1684:
      if (lookahead == 'a') ADVANCE(1787);
      END_STATE();
    case 1685:
      if (lookahead == 'a') ADVANCE(1788);
      END_STATE();
    case 1686:
      if (lookahead == 'r') ADVANCE(1789);
      END_STATE();
    case 1687:
      if (lookahead == 'r') ADVANCE(1790);
      END_STATE();
    case 1688:
      if (lookahead == 'a') ADVANCE(1791);
      END_STATE();
    case 1689:
      ACCEPT_TOKEN(anon_sym_VertexShader);
      END_STATE();
    case 1690:
      ACCEPT_TOKEN(anon_sym_asm_fragment);
      END_STATE();
    case 1691:
      if (lookahead == 'y') ADVANCE(1792);
      END_STATE();
    case 1692:
      ACCEPT_TOKEN(anon_sym_column_major);
      END_STATE();
    case 1693:
      if (lookahead == 'm') ADVANCE(1793);
      END_STATE();
    case 1694:
      if (lookahead == 'l') ADVANCE(1794);
      END_STATE();
    case 1695:
      ACCEPT_TOKEN(anon_sym_dynamic_cast);
      END_STATE();
    case 1696:
      if (lookahead == 'y') ADVANCE(1795);
      END_STATE();
    case 1697:
      if (lookahead == 'y') ADVANCE(1796);
      END_STATE();
    case 1698:
      if (lookahead == 'r') ADVANCE(1797);
      END_STATE();
    case 1699:
      ACCEPT_TOKEN(anon_sym_iimage2DRect);
      END_STATE();
    case 1700:
      ACCEPT_TOKEN(anon_sym_iimageBuffer);
      END_STATE();
    case 1701:
      if (lookahead == 'r') ADVANCE(1798);
      END_STATE();
    case 1702:
      ACCEPT_TOKEN(anon_sym_image1DArray);
      END_STATE();
    case 1703:
      ACCEPT_TOKEN(anon_sym_image2DArray);
      END_STATE();
    case 1704:
      if (lookahead == 'a') ADVANCE(1799);
      END_STATE();
    case 1705:
      if (lookahead == 'a') ADVANCE(1800);
      END_STATE();
    case 1706:
      if (lookahead == 'r') ADVANCE(1801);
      END_STATE();
    case 1707:
      if (lookahead == 'r') ADVANCE(1802);
      END_STATE();
    case 1708:
      ACCEPT_TOKEN(anon_sym_isampler2DMS);
      if (lookahead == 'A') ADVANCE(1803);
      END_STATE();
    case 1709:
      if (lookahead == 'c') ADVANCE(1804);
      END_STATE();
    case 1710:
      if (lookahead == 'e') ADVANCE(1805);
      END_STATE();
    case 1711:
      ACCEPT_TOKEN(anon_sym_isamplerCube);
      if (lookahead == 'A') ADVANCE(1806);
      END_STATE();
    case 1712:
      if (lookahead == 't') ADVANCE(1807);
      END_STATE();
    case 1713:
      if (lookahead == 'r') ADVANCE(1808);
      END_STATE();
    case 1714:
      if (lookahead == 'r') ADVANCE(1809);
      END_STATE();
    case 1715:
      ACCEPT_TOKEN(anon_sym_itexture2DMS);
      if (lookahead == 'A') ADVANCE(1810);
      END_STATE();
    case 1716:
      if (lookahead == 'c') ADVANCE(1811);
      END_STATE();
    case 1717:
      if (lookahead == 'e') ADVANCE(1812);
      END_STATE();
    case 1718:
      ACCEPT_TOKEN(anon_sym_itextureCube);
      if (lookahead == 'A') ADVANCE(1813);
      END_STATE();
    case 1719:
      if (lookahead == 'i') ADVANCE(1814);
      END_STATE();
    case 1720:
      if (lookahead == 'e') ADVANCE(1815);
      END_STATE();
    case 1721:
      if (lookahead == 't') ADVANCE(1816);
      END_STATE();
    case 1722:
      if (lookahead == 'c') ADVANCE(1817);
      END_STATE();
    case 1723:
      if (lookahead == 'a') ADVANCE(1818);
      END_STATE();
    case 1724:
      if (lookahead == 'd') ADVANCE(1819);
      END_STATE();
    case 1725:
      if (lookahead == 'a') ADVANCE(1820);
      END_STATE();
    case 1726:
      if (lookahead == 'r') ADVANCE(1821);
      END_STATE();
    case 1727:
      if (lookahead == 't') ADVANCE(1822);
      END_STATE();
    case 1728:
      if (lookahead == 'd') ADVANCE(1823);
      END_STATE();
    case 1729:
      if (lookahead == 't') ADVANCE(1824);
      END_STATE();
    case 1730:
      if (lookahead == 'r') ADVANCE(1825);
      END_STATE();
    case 1731:
      if (lookahead == 'r') ADVANCE(1826);
      END_STATE();
    case 1732:
      if (lookahead == 'h') ADVANCE(1827);
      END_STATE();
    case 1733:
      if (lookahead == 'w') ADVANCE(1828);
      END_STATE();
    case 1734:
      if (lookahead == 'a') ADVANCE(1829);
      END_STATE();
    case 1735:
      if (lookahead == 't') ADVANCE(1830);
      END_STATE();
    case 1736:
      ACCEPT_TOKEN(sym_staticAssert);
      END_STATE();
    case 1737:
      if (lookahead == 't') ADVANCE(1831);
      END_STATE();
    case 1738:
      ACCEPT_TOKEN(anon_sym_subpassInput);
      if (lookahead == 'M') ADVANCE(1832);
      END_STATE();
    case 1739:
      if (lookahead == 'a') ADVANCE(1833);
      END_STATE();
    case 1740:
      if (lookahead == 'a') ADVANCE(1834);
      END_STATE();
    case 1741:
      if (lookahead == 'r') ADVANCE(1835);
      END_STATE();
    case 1742:
      if (lookahead == 't') ADVANCE(1836);
      END_STATE();
    case 1743:
      if (lookahead == 'r') ADVANCE(1837);
      END_STATE();
    case 1744:
      if (lookahead == 'r') ADVANCE(1838);
      END_STATE();
    case 1745:
      if (lookahead == 'r') ADVANCE(1839);
      END_STATE();
    case 1746:
      ACCEPT_TOKEN(sym_texture_cube);
      if (lookahead == '_') ADVANCE(1840);
      END_STATE();
    case 1747:
      if (lookahead == 'h') ADVANCE(1841);
      END_STATE();
    case 1748:
      if (lookahead == 'i') ADVANCE(1842);
      END_STATE();
    case 1749:
      if (lookahead == 'a') ADVANCE(1843);
      END_STATE();
    case 1750:
      ACCEPT_TOKEN(anon_sym_thread_local);
      END_STATE();
    case 1751:
      if (lookahead == 'y') ADVANCE(1844);
      END_STATE();
    case 1752:
      if (lookahead == 'y') ADVANCE(1845);
      END_STATE();
    case 1753:
      if (lookahead == 'r') ADVANCE(1846);
      END_STATE();
    case 1754:
      ACCEPT_TOKEN(anon_sym_uimage2DRect);
      END_STATE();
    case 1755:
      ACCEPT_TOKEN(anon_sym_uimageBuffer);
      END_STATE();
    case 1756:
      if (lookahead == 'r') ADVANCE(1847);
      END_STATE();
    case 1757:
      if (lookahead == 'r') ADVANCE(1848);
      END_STATE();
    case 1758:
      if (lookahead == 'r') ADVANCE(1849);
      END_STATE();
    case 1759:
      ACCEPT_TOKEN(anon_sym_usampler2DMS);
      if (lookahead == 'A') ADVANCE(1850);
      END_STATE();
    case 1760:
      if (lookahead == 'c') ADVANCE(1851);
      END_STATE();
    case 1761:
      if (lookahead == 'e') ADVANCE(1852);
      END_STATE();
    case 1762:
      ACCEPT_TOKEN(anon_sym_usamplerCube);
      if (lookahead == 'A') ADVANCE(1853);
      END_STATE();
    case 1763:
      if (lookahead == 't') ADVANCE(1854);
      END_STATE();
    case 1764:
      if (lookahead == 'r') ADVANCE(1855);
      END_STATE();
    case 1765:
      if (lookahead == 'r') ADVANCE(1856);
      END_STATE();
    case 1766:
      ACCEPT_TOKEN(anon_sym_utexture2DMS);
      if (lookahead == 'A') ADVANCE(1857);
      END_STATE();
    case 1767:
      if (lookahead == 'c') ADVANCE(1858);
      END_STATE();
    case 1768:
      if (lookahead == 'e') ADVANCE(1859);
      END_STATE();
    case 1769:
      ACCEPT_TOKEN(anon_sym_utextureCube);
      if (lookahead == 'A') ADVANCE(1860);
      END_STATE();
    case 1770:
      if (lookahead == 'n') ADVANCE(1861);
      END_STATE();
    case 1771:
      if (lookahead == 'r') ADVANCE(1862);
      END_STATE();
    case 1772:
      if (lookahead == 'f') ADVANCE(1863);
      END_STATE();
    case 1773:
      ACCEPT_TOKEN(anon_sym_CompileShader);
      END_STATE();
    case 1774:
      ACCEPT_TOKEN(anon_sym_ComputeShader);
      END_STATE();
    case 1775:
      if (lookahead == 'u') ADVANCE(1864);
      END_STATE();
    case 1776:
      if (lookahead == 't') ADVANCE(1865);
      END_STATE();
    case 1777:
      if (lookahead == 'i') ADVANCE(1866);
      END_STATE();
    case 1778:
      if (lookahead == 'r') ADVANCE(1867);
      END_STATE();
    case 1779:
      if (lookahead == 'B') ADVANCE(1868);
      END_STATE();
    case 1780:
      if (lookahead == 'u') ADVANCE(1869);
      END_STATE();
    case 1781:
      if (lookahead == 'r') ADVANCE(1870);
      END_STATE();
    case 1782:
      if (lookahead == 'r') ADVANCE(1871);
      END_STATE();
    case 1783:
      if (lookahead == 't') ADVANCE(1872);
      END_STATE();
    case 1784:
      if (lookahead == 'i') ADVANCE(1873);
      END_STATE();
    case 1785:
      if (lookahead == 'i') ADVANCE(1874);
      END_STATE();
    case 1786:
      if (lookahead == 'f') ADVANCE(1875);
      END_STATE();
    case 1787:
      if (lookahead == 'y') ADVANCE(1876);
      END_STATE();
    case 1788:
      if (lookahead == 'y') ADVANCE(1877);
      END_STATE();
    case 1789:
      if (lookahead == 'r') ADVANCE(1878);
      END_STATE();
    case 1790:
      if (lookahead == 'r') ADVANCE(1879);
      END_STATE();
    case 1791:
      if (lookahead == 'm') ADVANCE(1880);
      END_STATE();
    case 1792:
      ACCEPT_TOKEN(anon_sym_binding_array);
      END_STATE();
    case 1793:
      if (lookahead == 'e') ADVANCE(1881);
      END_STATE();
    case 1794:
      if (lookahead == 'p') ADVANCE(1882);
      END_STATE();
    case 1795:
      ACCEPT_TOKEN(anon_sym_iimage1DArray);
      END_STATE();
    case 1796:
      ACCEPT_TOKEN(anon_sym_iimage2DArray);
      END_STATE();
    case 1797:
      if (lookahead == 'a') ADVANCE(1883);
      END_STATE();
    case 1798:
      if (lookahead == 'a') ADVANCE(1884);
      END_STATE();
    case 1799:
      if (lookahead == 'y') ADVANCE(1885);
      END_STATE();
    case 1800:
      if (lookahead == 'y') ADVANCE(1886);
      END_STATE();
    case 1801:
      if (lookahead == 'a') ADVANCE(1887);
      END_STATE();
    case 1802:
      if (lookahead == 'a') ADVANCE(1888);
      END_STATE();
    case 1803:
      if (lookahead == 'r') ADVANCE(1889);
      END_STATE();
    case 1804:
      if (lookahead == 't') ADVANCE(1890);
      END_STATE();
    case 1805:
      if (lookahead == 'r') ADVANCE(1891);
      END_STATE();
    case 1806:
      if (lookahead == 'r') ADVANCE(1892);
      END_STATE();
    case 1807:
      ACCEPT_TOKEN(anon_sym_isubpassInput);
      if (lookahead == 'M') ADVANCE(1893);
      END_STATE();
    case 1808:
      if (lookahead == 'a') ADVANCE(1894);
      END_STATE();
    case 1809:
      if (lookahead == 'a') ADVANCE(1895);
      END_STATE();
    case 1810:
      if (lookahead == 'r') ADVANCE(1896);
      END_STATE();
    case 1811:
      if (lookahead == 't') ADVANCE(1897);
      END_STATE();
    case 1812:
      if (lookahead == 'r') ADVANCE(1898);
      END_STATE();
    case 1813:
      if (lookahead == 'r') ADVANCE(1899);
      END_STATE();
    case 1814:
      if (lookahead == 'o') ADVANCE(1900);
      END_STATE();
    case 1815:
      ACCEPT_TOKEN(anon_sym_noperspective);
      END_STATE();
    case 1816:
      ACCEPT_TOKEN(anon_sym_pixelfragment);
      END_STATE();
    case 1817:
      if (lookahead == 'a') ADVANCE(1901);
      END_STATE();
    case 1818:
      if (lookahead == 'y') ADVANCE(1902);
      END_STATE();
    case 1819:
      if (lookahead == 'o') ADVANCE(1903);
      END_STATE();
    case 1820:
      if (lookahead == 'y') ADVANCE(1904);
      END_STATE();
    case 1821:
      if (lookahead == 'r') ADVANCE(1905);
      END_STATE();
    case 1822:
      ACCEPT_TOKEN(anon_sym_sampler2DRect);
      if (lookahead == 'S') ADVANCE(1906);
      END_STATE();
    case 1823:
      if (lookahead == 'o') ADVANCE(1907);
      END_STATE();
    case 1824:
      ACCEPT_TOKEN(anon_sym_sampler3DRect);
      END_STATE();
    case 1825:
      ACCEPT_TOKEN(anon_sym_samplerBuffer);
      END_STATE();
    case 1826:
      if (lookahead == 'r') ADVANCE(1908);
      END_STATE();
    case 1827:
      if (lookahead == 'a') ADVANCE(1909);
      END_STATE();
    case 1828:
      ACCEPT_TOKEN(anon_sym_samplerShadow);
      END_STATE();
    case 1829:
      if (lookahead == 'r') ADVANCE(1910);
      END_STATE();
    case 1830:
      if (lookahead == 'a') ADVANCE(1911);
      END_STATE();
    case 1831:
      ACCEPT_TOKEN(anon_sym_static_assert);
      END_STATE();
    case 1832:
      if (lookahead == 'S') ADVANCE(1912);
      END_STATE();
    case 1833:
      if (lookahead == 'y') ADVANCE(1913);
      END_STATE();
    case 1834:
      if (lookahead == 'y') ADVANCE(1914);
      END_STATE();
    case 1835:
      if (lookahead == 'r') ADVANCE(1915);
      END_STATE();
    case 1836:
      ACCEPT_TOKEN(anon_sym_texture2DRect);
      END_STATE();
    case 1837:
      ACCEPT_TOKEN(anon_sym_textureBuffer);
      END_STATE();
    case 1838:
      if (lookahead == 'r') ADVANCE(1916);
      END_STATE();
    case 1839:
      if (lookahead == 'r') ADVANCE(1917);
      END_STATE();
    case 1840:
      if (lookahead == 'a') ADVANCE(1918);
      END_STATE();
    case 1841:
      if (lookahead == '_') ADVANCE(1919);
      END_STATE();
    case 1842:
      if (lookahead == 's') ADVANCE(1920);
      END_STATE();
    case 1843:
      if (lookahead == 'g') ADVANCE(1921);
      END_STATE();
    case 1844:
      ACCEPT_TOKEN(anon_sym_uimage1DArray);
      END_STATE();
    case 1845:
      ACCEPT_TOKEN(anon_sym_uimage2DArray);
      END_STATE();
    case 1846:
      if (lookahead == 'a') ADVANCE(1922);
      END_STATE();
    case 1847:
      if (lookahead == 'a') ADVANCE(1923);
      END_STATE();
    case 1848:
      if (lookahead == 'a') ADVANCE(1924);
      END_STATE();
    case 1849:
      if (lookahead == 'a') ADVANCE(1925);
      END_STATE();
    case 1850:
      if (lookahead == 'r') ADVANCE(1926);
      END_STATE();
    case 1851:
      if (lookahead == 't') ADVANCE(1927);
      END_STATE();
    case 1852:
      if (lookahead == 'r') ADVANCE(1928);
      END_STATE();
    case 1853:
      if (lookahead == 'r') ADVANCE(1929);
      END_STATE();
    case 1854:
      ACCEPT_TOKEN(anon_sym_usubpassInput);
      if (lookahead == 'M') ADVANCE(1930);
      END_STATE();
    case 1855:
      if (lookahead == 'a') ADVANCE(1931);
      END_STATE();
    case 1856:
      if (lookahead == 'a') ADVANCE(1932);
      END_STATE();
    case 1857:
      if (lookahead == 'r') ADVANCE(1933);
      END_STATE();
    case 1858:
      if (lookahead == 't') ADVANCE(1934);
      END_STATE();
    case 1859:
      if (lookahead == 'r') ADVANCE(1935);
      END_STATE();
    case 1860:
      if (lookahead == 'r') ADVANCE(1936);
      END_STATE();
    case 1861:
      if (lookahead == 't') ADVANCE(1937);
      END_STATE();
    case 1862:
      if (lookahead == 'e') ADVANCE(1938);
      END_STATE();
    case 1863:
      if (lookahead == 'f') ADVANCE(1939);
      END_STATE();
    case 1864:
      if (lookahead == 'r') ADVANCE(1940);
      END_STATE();
    case 1865:
      if (lookahead == 'a') ADVANCE(1941);
      END_STATE();
    case 1866:
      if (lookahead == 'e') ADVANCE(1942);
      END_STATE();
    case 1867:
      ACCEPT_TOKEN(anon_sym_GeometryShader);
      END_STATE();
    case 1868:
      if (lookahead == 'u') ADVANCE(1943);
      END_STATE();
    case 1869:
      if (lookahead == 'f') ADVANCE(1944);
      END_STATE();
    case 1870:
      if (lookahead == 'a') ADVANCE(1945);
      END_STATE();
    case 1871:
      if (lookahead == 'a') ADVANCE(1946);
      END_STATE();
    case 1872:
      if (lookahead == 'e') ADVANCE(1947);
      END_STATE();
    case 1873:
      if (lookahead == 'e') ADVANCE(1948);
      END_STATE();
    case 1874:
      if (lookahead == 's') ADVANCE(1949);
      END_STATE();
    case 1875:
      if (lookahead == 'e') ADVANCE(1950);
      END_STATE();
    case 1876:
      ACCEPT_TOKEN(anon_sym_Texture1DArray);
      END_STATE();
    case 1877:
      ACCEPT_TOKEN(anon_sym_Texture2DArray);
      END_STATE();
    case 1878:
      if (lookahead == 'a') ADVANCE(1951);
      END_STATE();
    case 1879:
      if (lookahead == 'a') ADVANCE(1952);
      END_STATE();
    case 1880:
      ACCEPT_TOKEN(anon_sym_TriangleStream);
      END_STATE();
    case 1881:
      if (lookahead == 'n') ADVANCE(1953);
      END_STATE();
    case 1882:
      if (lookahead == 'e') ADVANCE(1954);
      END_STATE();
    case 1883:
      if (lookahead == 'y') ADVANCE(1955);
      END_STATE();
    case 1884:
      if (lookahead == 'y') ADVANCE(1956);
      END_STATE();
    case 1885:
      ACCEPT_TOKEN(anon_sym_image2DMSArray);
      END_STATE();
    case 1886:
      ACCEPT_TOKEN(anon_sym_imageCubeArray);
      END_STATE();
    case 1887:
      if (lookahead == 'y') ADVANCE(1957);
      END_STATE();
    case 1888:
      if (lookahead == 'y') ADVANCE(1958);
      END_STATE();
    case 1889:
      if (lookahead == 'r') ADVANCE(1959);
      END_STATE();
    case 1890:
      ACCEPT_TOKEN(anon_sym_isampler2DRect);
      END_STATE();
    case 1891:
      ACCEPT_TOKEN(anon_sym_isamplerBuffer);
      END_STATE();
    case 1892:
      if (lookahead == 'r') ADVANCE(1960);
      END_STATE();
    case 1893:
      if (lookahead == 'S') ADVANCE(1961);
      END_STATE();
    case 1894:
      if (lookahead == 'y') ADVANCE(1962);
      END_STATE();
    case 1895:
      if (lookahead == 'y') ADVANCE(1963);
      END_STATE();
    case 1896:
      if (lookahead == 'r') ADVANCE(1964);
      END_STATE();
    case 1897:
      ACCEPT_TOKEN(anon_sym_itexture2DRect);
      END_STATE();
    case 1898:
      ACCEPT_TOKEN(anon_sym_itextureBuffer);
      END_STATE();
    case 1899:
      if (lookahead == 'r') ADVANCE(1965);
      END_STATE();
    case 1900:
      if (lookahead == 'n') ADVANCE(1966);
      END_STATE();
    case 1901:
      if (lookahead == 's') ADVANCE(1967);
      END_STATE();
    case 1902:
      ACCEPT_TOKEN(anon_sym_sampler1DArray);
      if (lookahead == 'S') ADVANCE(1968);
      END_STATE();
    case 1903:
      if (lookahead == 'w') ADVANCE(1969);
      END_STATE();
    case 1904:
      ACCEPT_TOKEN(anon_sym_sampler2DArray);
      if (lookahead == 'S') ADVANCE(1970);
      END_STATE();
    case 1905:
      if (lookahead == 'a') ADVANCE(1971);
      END_STATE();
    case 1906:
      if (lookahead == 'h') ADVANCE(1972);
      END_STATE();
    case 1907:
      if (lookahead == 'w') ADVANCE(1973);
      END_STATE();
    case 1908:
      if (lookahead == 'a') ADVANCE(1974);
      END_STATE();
    case 1909:
      if (lookahead == 'd') ADVANCE(1975);
      END_STATE();
    case 1910:
      if (lookahead == 'i') ADVANCE(1976);
      END_STATE();
    case 1911:
      if (lookahead == 't') ADVANCE(1977);
      END_STATE();
    case 1912:
      ACCEPT_TOKEN(anon_sym_subpassInputMS);
      END_STATE();
    case 1913:
      ACCEPT_TOKEN(anon_sym_texture1DArray);
      END_STATE();
    case 1914:
      ACCEPT_TOKEN(anon_sym_texture2DArray);
      END_STATE();
    case 1915:
      if (lookahead == 'a') ADVANCE(1978);
      END_STATE();
    case 1916:
      if (lookahead == 'a') ADVANCE(1979);
      END_STATE();
    case 1917:
      if (lookahead == 'a') ADVANCE(1980);
      END_STATE();
    case 1918:
      if (lookahead == 'r') ADVANCE(1981);
      END_STATE();
    case 1919:
      if (lookahead == '2') ADVANCE(1982);
      if (lookahead == 'c') ADVANCE(1983);
      if (lookahead == 'm') ADVANCE(1984);
      END_STATE();
    case 1920:
      if (lookahead == 'a') ADVANCE(1985);
      END_STATE();
    case 1921:
      if (lookahead == 'e') ADVANCE(1986);
      END_STATE();
    case 1922:
      if (lookahead == 'y') ADVANCE(1987);
      END_STATE();
    case 1923:
      if (lookahead == 'y') ADVANCE(1988);
      END_STATE();
    case 1924:
      if (lookahead == 'y') ADVANCE(1989);
      END_STATE();
    case 1925:
      if (lookahead == 'y') ADVANCE(1990);
      END_STATE();
    case 1926:
      if (lookahead == 'r') ADVANCE(1991);
      END_STATE();
    case 1927:
      ACCEPT_TOKEN(anon_sym_usampler2DRect);
      END_STATE();
    case 1928:
      ACCEPT_TOKEN(anon_sym_usamplerBuffer);
      END_STATE();
    case 1929:
      if (lookahead == 'r') ADVANCE(1992);
      END_STATE();
    case 1930:
      if (lookahead == 'S') ADVANCE(1993);
      END_STATE();
    case 1931:
      if (lookahead == 'y') ADVANCE(1994);
      END_STATE();
    case 1932:
      if (lookahead == 'y') ADVANCE(1995);
      END_STATE();
    case 1933:
      if (lookahead == 'r') ADVANCE(1996);
      END_STATE();
    case 1934:
      ACCEPT_TOKEN(anon_sym_utexture2DRect);
      END_STATE();
    case 1935:
      ACCEPT_TOKEN(anon_sym_utextureBuffer);
      END_STATE();
    case 1936:
      if (lookahead == 'r') ADVANCE(1997);
      END_STATE();
    case 1937:
      ACCEPT_TOKEN(anon_sym_vertexfragment);
      END_STATE();
    case 1938:
      if (lookahead == 'd') ADVANCE(1998);
      END_STATE();
    case 1939:
      if (lookahead == 'e') ADVANCE(1999);
      END_STATE();
    case 1940:
      if (lookahead == 'e') ADVANCE(2000);
      END_STATE();
    case 1941:
      if (lookahead == 't') ADVANCE(2001);
      END_STATE();
    case 1942:
      if (lookahead == 'w') ADVANCE(2002);
      END_STATE();
    case 1943:
      if (lookahead == 'f') ADVANCE(2003);
      END_STATE();
    case 1944:
      if (lookahead == 'f') ADVANCE(2004);
      END_STATE();
    case 1945:
      if (lookahead == 'y') ADVANCE(2005);
      END_STATE();
    case 1946:
      if (lookahead == 'y') ADVANCE(2006);
      END_STATE();
    case 1947:
      ACCEPT_TOKEN(anon_sym_RasterizerState);
      END_STATE();
    case 1948:
      if (lookahead == 'w') ADVANCE(2007);
      END_STATE();
    case 1949:
      if (lookahead == 'o') ADVANCE(2008);
      END_STATE();
    case 1950:
      if (lookahead == 'r') ADVANCE(2009);
      END_STATE();
    case 1951:
      if (lookahead == 'y') ADVANCE(2010);
      END_STATE();
    case 1952:
      if (lookahead == 'y') ADVANCE(2011);
      END_STATE();
    case 1953:
      if (lookahead == 't') ADVANCE(2012);
      END_STATE();
    case 1954:
      if (lookahead == 'r') ADVANCE(2013);
      END_STATE();
    case 1955:
      ACCEPT_TOKEN(anon_sym_iimage2DMSArray);
      END_STATE();
    case 1956:
      ACCEPT_TOKEN(anon_sym_iimageCubeArray);
      END_STATE();
    case 1957:
      ACCEPT_TOKEN(anon_sym_isampler1DArray);
      END_STATE();
    case 1958:
      ACCEPT_TOKEN(anon_sym_isampler2DArray);
      END_STATE();
    case 1959:
      if (lookahead == 'a') ADVANCE(2014);
      END_STATE();
    case 1960:
      if (lookahead == 'a') ADVANCE(2015);
      END_STATE();
    case 1961:
      ACCEPT_TOKEN(anon_sym_isubpassInputMS);
      END_STATE();
    case 1962:
      ACCEPT_TOKEN(anon_sym_itexture1DArray);
      END_STATE();
    case 1963:
      ACCEPT_TOKEN(anon_sym_itexture2DArray);
      END_STATE();
    case 1964:
      if (lookahead == 'a') ADVANCE(2016);
      END_STATE();
    case 1965:
      if (lookahead == 'a') ADVANCE(2017);
      END_STATE();
    case 1966:
      ACCEPT_TOKEN(anon_sym_nointerpolation);
      END_STATE();
    case 1967:
      if (lookahead == 't') ADVANCE(2018);
      END_STATE();
    case 1968:
      if (lookahead == 'h') ADVANCE(2019);
      END_STATE();
    case 1969:
      ACCEPT_TOKEN(anon_sym_sampler1DShadow);
      END_STATE();
    case 1970:
      if (lookahead == 'h') ADVANCE(2020);
      END_STATE();
    case 1971:
      if (lookahead == 'y') ADVANCE(2021);
      END_STATE();
    case 1972:
      if (lookahead == 'a') ADVANCE(2022);
      END_STATE();
    case 1973:
      ACCEPT_TOKEN(anon_sym_sampler2DShadow);
      END_STATE();
    case 1974:
      if (lookahead == 'y') ADVANCE(2023);
      END_STATE();
    case 1975:
      if (lookahead == 'o') ADVANCE(2024);
      END_STATE();
    case 1976:
      if (lookahead == 's') ADVANCE(2025);
      END_STATE();
    case 1977:
      if (lookahead == 'e') ADVANCE(2026);
      END_STATE();
    case 1978:
      if (lookahead == 'y') ADVANCE(2027);
      END_STATE();
    case 1979:
      if (lookahead == 'y') ADVANCE(2028);
      END_STATE();
    case 1980:
      if (lookahead == 'y') ADVANCE(2029);
      END_STATE();
    case 1981:
      if (lookahead == 'r') ADVANCE(2030);
      END_STATE();
    case 1982:
      if (lookahead == 'd') ADVANCE(2031);
      END_STATE();
    case 1983:
      if (lookahead == 'u') ADVANCE(2032);
      END_STATE();
    case 1984:
      if (lookahead == 'u') ADVANCE(2033);
      END_STATE();
    case 1985:
      if (lookahead == 'm') ADVANCE(2034);
      END_STATE();
    case 1986:
      if (lookahead == '_') ADVANCE(2035);
      END_STATE();
    case 1987:
      ACCEPT_TOKEN(anon_sym_uimage2DMSArray);
      END_STATE();
    case 1988:
      ACCEPT_TOKEN(anon_sym_uimageCubeArray);
      END_STATE();
    case 1989:
      ACCEPT_TOKEN(anon_sym_usampler1DArray);
      END_STATE();
    case 1990:
      ACCEPT_TOKEN(anon_sym_usampler2DArray);
      END_STATE();
    case 1991:
      if (lookahead == 'a') ADVANCE(2036);
      END_STATE();
    case 1992:
      if (lookahead == 'a') ADVANCE(2037);
      END_STATE();
    case 1993:
      ACCEPT_TOKEN(anon_sym_usubpassInputMS);
      END_STATE();
    case 1994:
      ACCEPT_TOKEN(anon_sym_utexture1DArray);
      END_STATE();
    case 1995:
      ACCEPT_TOKEN(anon_sym_utexture2DArray);
      END_STATE();
    case 1996:
      if (lookahead == 'a') ADVANCE(2038);
      END_STATE();
    case 1997:
      if (lookahead == 'a') ADVANCE(2039);
      END_STATE();
    case 1998:
      if (lookahead == 'B') ADVANCE(2040);
      END_STATE();
    case 1999:
      if (lookahead == 'r') ADVANCE(2041);
      END_STATE();
    case 2000:
      if (lookahead == 'd') ADVANCE(2042);
      END_STATE();
    case 2001:
      if (lookahead == 'e') ADVANCE(2043);
      END_STATE();
    case 2002:
      ACCEPT_TOKEN(anon_sym_DepthStencilView);
      END_STATE();
    case 2003:
      if (lookahead == 'f') ADVANCE(2044);
      END_STATE();
    case 2004:
      if (lookahead == 'e') ADVANCE(2045);
      END_STATE();
    case 2005:
      ACCEPT_TOKEN(anon_sym_RWTexture1DArray);
      END_STATE();
    case 2006:
      ACCEPT_TOKEN(anon_sym_RWTexture2DArray);
      END_STATE();
    case 2007:
      ACCEPT_TOKEN(anon_sym_RenderTargetView);
      END_STATE();
    case 2008:
      if (lookahead == 'n') ADVANCE(2046);
      END_STATE();
    case 2009:
      ACCEPT_TOKEN(anon_sym_StructuredBuffer);
      END_STATE();
    case 2010:
      ACCEPT_TOKEN(anon_sym_Texture2DMSArray);
      END_STATE();
    case 2011:
      ACCEPT_TOKEN(anon_sym_TextureCubeArray);
      END_STATE();
    case 2012:
      ACCEPT_TOKEN(anon_sym_compile_fragment);
      END_STATE();
    case 2013:
      ACCEPT_TOKEN(anon_sym_demote_to_helper);
      END_STATE();
    case 2014:
      if (lookahead == 'y') ADVANCE(2047);
      END_STATE();
    case 2015:
      if (lookahead == 'y') ADVANCE(2048);
      END_STATE();
    case 2016:
      if (lookahead == 'y') ADVANCE(2049);
      END_STATE();
    case 2017:
      if (lookahead == 'y') ADVANCE(2050);
      END_STATE();
    case 2018:
      ACCEPT_TOKEN(anon_sym_reinterpret_cast);
      END_STATE();
    case 2019:
      if (lookahead == 'a') ADVANCE(2051);
      END_STATE();
    case 2020:
      if (lookahead == 'a') ADVANCE(2052);
      END_STATE();
    case 2021:
      ACCEPT_TOKEN(anon_sym_sampler2DMSArray);
      END_STATE();
    case 2022:
      if (lookahead == 'd') ADVANCE(2053);
      END_STATE();
    case 2023:
      ACCEPT_TOKEN(anon_sym_samplerCubeArray);
      if (lookahead == 'S') ADVANCE(2054);
      END_STATE();
    case 2024:
      if (lookahead == 'w') ADVANCE(2055);
      END_STATE();
    case 2025:
      if (lookahead == 'o') ADVANCE(2056);
      END_STATE();
    case 2026:
      ACCEPT_TOKEN(anon_sym_stateblock_state);
      END_STATE();
    case 2027:
      ACCEPT_TOKEN(anon_sym_texture2DMSArray);
      END_STATE();
    case 2028:
      ACCEPT_TOKEN(anon_sym_textureCubeArray);
      END_STATE();
    case 2029:
      ACCEPT_TOKEN(sym_texture_2d_array);
      END_STATE();
    case 2030:
      if (lookahead == 'a') ADVANCE(2057);
      END_STATE();
    case 2031:
      ACCEPT_TOKEN(sym_texture_depth_2d);
      if (lookahead == '_') ADVANCE(2058);
      END_STATE();
    case 2032:
      if (lookahead == 'b') ADVANCE(2059);
      END_STATE();
    case 2033:
      if (lookahead == 'l') ADVANCE(2060);
      END_STATE();
    case 2034:
      if (lookahead == 'p') ADVANCE(2061);
      END_STATE();
    case 2035:
      if (lookahead == '1') ADVANCE(2062);
      if (lookahead == '2') ADVANCE(2063);
      if (lookahead == '3') ADVANCE(2064);
      END_STATE();
    case 2036:
      if (lookahead == 'y') ADVANCE(2065);
      END_STATE();
    case 2037:
      if (lookahead == 'y') ADVANCE(2066);
      END_STATE();
    case 2038:
      if (lookahead == 'y') ADVANCE(2067);
      END_STATE();
    case 2039:
      if (lookahead == 'y') ADVANCE(2068);
      END_STATE();
    case 2040:
      if (lookahead == 'u') ADVANCE(2069);
      END_STATE();
    case 2041:
      ACCEPT_TOKEN(anon_sym_ByteAddressBuffer);
      END_STATE();
    case 2042:
      if (lookahead == 'B') ADVANCE(2070);
      END_STATE();
    case 2043:
      ACCEPT_TOKEN(anon_sym_DepthStencilState);
      END_STATE();
    case 2044:
      if (lookahead == 'e') ADVANCE(2071);
      END_STATE();
    case 2045:
      if (lookahead == 'r') ADVANCE(2072);
      END_STATE();
    case 2046:
      if (lookahead == 'S') ADVANCE(2073);
      END_STATE();
    case 2047:
      ACCEPT_TOKEN(anon_sym_isampler2DMSArray);
      END_STATE();
    case 2048:
      ACCEPT_TOKEN(anon_sym_isamplerCubeArray);
      END_STATE();
    case 2049:
      ACCEPT_TOKEN(anon_sym_itexture2DMSArray);
      END_STATE();
    case 2050:
      ACCEPT_TOKEN(anon_sym_itextureCubeArray);
      END_STATE();
    case 2051:
      if (lookahead == 'd') ADVANCE(2074);
      END_STATE();
    case 2052:
      if (lookahead == 'd') ADVANCE(2075);
      END_STATE();
    case 2053:
      if (lookahead == 'o') ADVANCE(2076);
      END_STATE();
    case 2054:
      if (lookahead == 'h') ADVANCE(2077);
      END_STATE();
    case 2055:
      ACCEPT_TOKEN(anon_sym_samplerCubeShadow);
      END_STATE();
    case 2056:
      if (lookahead == 'n') ADVANCE(2078);
      END_STATE();
    case 2057:
      if (lookahead == 'y') ADVANCE(2079);
      END_STATE();
    case 2058:
      if (lookahead == 'a') ADVANCE(2080);
      END_STATE();
    case 2059:
      if (lookahead == 'e') ADVANCE(2081);
      END_STATE();
    case 2060:
      if (lookahead == 't') ADVANCE(2082);
      END_STATE();
    case 2061:
      if (lookahead == 'l') ADVANCE(2083);
      END_STATE();
    case 2062:
      if (lookahead == 'd') ADVANCE(2084);
      END_STATE();
    case 2063:
      if (lookahead == 'd') ADVANCE(2085);
      END_STATE();
    case 2064:
      if (lookahead == 'd') ADVANCE(2086);
      END_STATE();
    case 2065:
      ACCEPT_TOKEN(anon_sym_usampler2DMSArray);
      END_STATE();
    case 2066:
      ACCEPT_TOKEN(anon_sym_usamplerCubeArray);
      END_STATE();
    case 2067:
      ACCEPT_TOKEN(anon_sym_utexture2DMSArray);
      END_STATE();
    case 2068:
      ACCEPT_TOKEN(anon_sym_utextureCubeArray);
      END_STATE();
    case 2069:
      if (lookahead == 'f') ADVANCE(2087);
      END_STATE();
    case 2070:
      if (lookahead == 'u') ADVANCE(2088);
      END_STATE();
    case 2071:
      if (lookahead == 'r') ADVANCE(2089);
      END_STATE();
    case 2072:
      ACCEPT_TOKEN(anon_sym_RWStructuredBuffer);
      END_STATE();
    case 2073:
      if (lookahead == 't') ADVANCE(2090);
      END_STATE();
    case 2074:
      if (lookahead == 'o') ADVANCE(2091);
      END_STATE();
    case 2075:
      if (lookahead == 'o') ADVANCE(2092);
      END_STATE();
    case 2076:
      if (lookahead == 'w') ADVANCE(2093);
      END_STATE();
    case 2077:
      if (lookahead == 'a') ADVANCE(2094);
      END_STATE();
    case 2078:
      ACCEPT_TOKEN(sym_sampler_comparison);
      END_STATE();
    case 2079:
      ACCEPT_TOKEN(sym_texture_cube_array);
      END_STATE();
    case 2080:
      if (lookahead == 'r') ADVANCE(2095);
      END_STATE();
    case 2081:
      ACCEPT_TOKEN(sym_texture_depth_cube);
      if (lookahead == '_') ADVANCE(2096);
      END_STATE();
    case 2082:
      if (lookahead == 'i') ADVANCE(2097);
      END_STATE();
    case 2083:
      if (lookahead == 'e') ADVANCE(2098);
      END_STATE();
    case 2084:
      ACCEPT_TOKEN(sym_texture_storage_1d);
      END_STATE();
    case 2085:
      ACCEPT_TOKEN(sym_texture_storage_2d);
      if (lookahead == '_') ADVANCE(2099);
      END_STATE();
    case 2086:
      ACCEPT_TOKEN(sym_texture_storage_3d);
      END_STATE();
    case 2087:
      if (lookahead == 'f') ADVANCE(2100);
      END_STATE();
    case 2088:
      if (lookahead == 'f') ADVANCE(2101);
      END_STATE();
    case 2089:
      ACCEPT_TOKEN(anon_sym_RWByteAddressBuffer);
      END_STATE();
    case 2090:
      if (lookahead == 'a') ADVANCE(2102);
      END_STATE();
    case 2091:
      if (lookahead == 'w') ADVANCE(2103);
      END_STATE();
    case 2092:
      if (lookahead == 'w') ADVANCE(2104);
      END_STATE();
    case 2093:
      ACCEPT_TOKEN(anon_sym_sampler2DRectShadow);
      END_STATE();
    case 2094:
      if (lookahead == 'd') ADVANCE(2105);
      END_STATE();
    case 2095:
      if (lookahead == 'r') ADVANCE(2106);
      END_STATE();
    case 2096:
      if (lookahead == 'a') ADVANCE(2107);
      END_STATE();
    case 2097:
      if (lookahead == 's') ADVANCE(2108);
      END_STATE();
    case 2098:
      if (lookahead == 'd') ADVANCE(2109);
      END_STATE();
    case 2099:
      if (lookahead == 'a') ADVANCE(2110);
      END_STATE();
    case 2100:
      if (lookahead == 'e') ADVANCE(2111);
      END_STATE();
    case 2101:
      if (lookahead == 'f') ADVANCE(2112);
      END_STATE();
    case 2102:
      if (lookahead == 't') ADVANCE(2113);
      END_STATE();
    case 2103:
      ACCEPT_TOKEN(anon_sym_sampler1DArrayShadow);
      END_STATE();
    case 2104:
      ACCEPT_TOKEN(anon_sym_sampler2DArrayShadow);
      END_STATE();
    case 2105:
      if (lookahead == 'o') ADVANCE(2114);
      END_STATE();
    case 2106:
      if (lookahead == 'a') ADVANCE(2115);
      END_STATE();
    case 2107:
      if (lookahead == 'r') ADVANCE(2116);
      END_STATE();
    case 2108:
      if (lookahead == 'a') ADVANCE(2117);
      END_STATE();
    case 2109:
      if (lookahead == '_') ADVANCE(2118);
      END_STATE();
    case 2110:
      if (lookahead == 'r') ADVANCE(2119);
      END_STATE();
    case 2111:
      if (lookahead == 'r') ADVANCE(2120);
      END_STATE();
    case 2112:
      if (lookahead == 'e') ADVANCE(2121);
      END_STATE();
    case 2113:
      if (lookahead == 'e') ADVANCE(2122);
      END_STATE();
    case 2114:
      if (lookahead == 'w') ADVANCE(2123);
      END_STATE();
    case 2115:
      if (lookahead == 'y') ADVANCE(2124);
      END_STATE();
    case 2116:
      if (lookahead == 'r') ADVANCE(2125);
      END_STATE();
    case 2117:
      if (lookahead == 'm') ADVANCE(2126);
      END_STATE();
    case 2118:
      if (lookahead == '2') ADVANCE(2127);
      END_STATE();
    case 2119:
      if (lookahead == 'r') ADVANCE(2128);
      END_STATE();
    case 2120:
      ACCEPT_TOKEN(anon_sym_AppendStructuredBuffer);
      END_STATE();
    case 2121:
      if (lookahead == 'r') ADVANCE(2129);
      END_STATE();
    case 2122:
      ACCEPT_TOKEN(anon_sym_SamplerComparisonState);
      END_STATE();
    case 2123:
      ACCEPT_TOKEN(anon_sym_samplerCubeArrayShadow);
      END_STATE();
    case 2124:
      ACCEPT_TOKEN(sym_texture_depth_2d_array);
      END_STATE();
    case 2125:
      if (lookahead == 'a') ADVANCE(2130);
      END_STATE();
    case 2126:
      if (lookahead == 'p') ADVANCE(2131);
      END_STATE();
    case 2127:
      if (lookahead == 'd') ADVANCE(2132);
      END_STATE();
    case 2128:
      if (lookahead == 'a') ADVANCE(2133);
      END_STATE();
    case 2129:
      ACCEPT_TOKEN(anon_sym_ConsumeStructuredBuffer);
      END_STATE();
    case 2130:
      if (lookahead == 'y') ADVANCE(2134);
      END_STATE();
    case 2131:
      if (lookahead == 'l') ADVANCE(2135);
      END_STATE();
    case 2132:
      ACCEPT_TOKEN(sym_texture_multisampled_2d);
      END_STATE();
    case 2133:
      if (lookahead == 'y') ADVANCE(2136);
      END_STATE();
    case 2134:
      ACCEPT_TOKEN(sym_texture_depth_cube_array);
      END_STATE();
    case 2135:
      if (lookahead == 'e') ADVANCE(2137);
      END_STATE();
    case 2136:
      ACCEPT_TOKEN(sym_texture_storage_2d_array);
      END_STATE();
    case 2137:
      if (lookahead == 'd') ADVANCE(2138);
      END_STATE();
    case 2138:
      if (lookahead == '_') ADVANCE(2139);
      END_STATE();
    case 2139:
      if (lookahead == '2') ADVANCE(2140);
      END_STATE();
    case 2140:
      if (lookahead == 'd') ADVANCE(2141);
      END_STATE();
    case 2141:
      ACCEPT_TOKEN(sym_texture_depth_multisampled_2d);
      END_STATE();
    default:
      return false;
  }
}

static const TSLexMode ts_lex_modes[STATE_COUNT] = {
  [0] = {.lex_state = 0, .external_lex_state = 1},
  [1] = {.lex_state = 0, .external_lex_state = 1},
  [2] = {.lex_state = 32, .external_lex_state = 1},
  [3] = {.lex_state = 32, .external_lex_state = 1},
  [4] = {.lex_state = 32, .external_lex_state = 1},
  [5] = {.lex_state = 32, .external_lex_state = 1},
  [6] = {.lex_state = 32, .external_lex_state = 1},
  [7] = {.lex_state = 32, .external_lex_state = 1},
  [8] = {.lex_state = 32, .external_lex_state = 1},
  [9] = {.lex_state = 32, .external_lex_state = 1},
  [10] = {.lex_state = 32, .external_lex_state = 1},
  [11] = {.lex_state = 32, .external_lex_state = 1},
  [12] = {.lex_state = 32, .external_lex_state = 1},
  [13] = {.lex_state = 32, .external_lex_state = 1},
  [14] = {.lex_state = 32, .external_lex_state = 1},
  [15] = {.lex_state = 32, .external_lex_state = 1},
  [16] = {.lex_state = 32, .external_lex_state = 1},
  [17] = {.lex_state = 32, .external_lex_state = 1},
  [18] = {.lex_state = 32, .external_lex_state = 1},
  [19] = {.lex_state = 32, .external_lex_state = 1},
  [20] = {.lex_state = 32, .external_lex_state = 1},
  [21] = {.lex_state = 32, .external_lex_state = 1},
  [22] = {.lex_state = 32, .external_lex_state = 1},
  [23] = {.lex_state = 32, .external_lex_state = 1},
  [24] = {.lex_state = 32, .external_lex_state = 1},
  [25] = {.lex_state = 32, .external_lex_state = 1},
  [26] = {.lex_state = 32, .external_lex_state = 1},
  [27] = {.lex_state = 32, .external_lex_state = 1},
  [28] = {.lex_state = 32, .external_lex_state = 1},
  [29] = {.lex_state = 32, .external_lex_state = 1},
  [30] = {.lex_state = 32, .external_lex_state = 1},
  [31] = {.lex_state = 32, .external_lex_state = 1},
  [32] = {.lex_state = 32, .external_lex_state = 1},
  [33] = {.lex_state = 32, .external_lex_state = 1},
  [34] = {.lex_state = 32, .external_lex_state = 1},
  [35] = {.lex_state = 32, .external_lex_state = 1},
  [36] = {.lex_state = 32, .external_lex_state = 1},
  [37] = {.lex_state = 32, .external_lex_state = 1},
  [38] = {.lex_state = 32, .external_lex_state = 1},
  [39] = {.lex_state = 32, .external_lex_state = 1},
  [40] = {.lex_state = 32, .external_lex_state = 1},
  [41] = {.lex_state = 32, .external_lex_state = 1},
  [42] = {.lex_state = 32, .external_lex_state = 1},
  [43] = {.lex_state = 32, .external_lex_state = 1},
  [44] = {.lex_state = 32, .external_lex_state = 1},
  [45] = {.lex_state = 32, .external_lex_state = 1},
  [46] = {.lex_state = 32, .external_lex_state = 1},
  [47] = {.lex_state = 32, .external_lex_state = 1},
  [48] = {.lex_state = 32, .external_lex_state = 1},
  [49] = {.lex_state = 32, .external_lex_state = 1},
  [50] = {.lex_state = 32, .external_lex_state = 1},
  [51] = {.lex_state = 32, .external_lex_state = 1},
  [52] = {.lex_state = 32, .external_lex_state = 1},
  [53] = {.lex_state = 0, .external_lex_state = 1},
  [54] = {.lex_state = 0, .external_lex_state = 1},
  [55] = {.lex_state = 0, .external_lex_state = 1},
  [56] = {.lex_state = 0, .external_lex_state = 1},
  [57] = {.lex_state = 0, .external_lex_state = 1},
  [58] = {.lex_state = 0, .external_lex_state = 1},
  [59] = {.lex_state = 0, .external_lex_state = 1},
  [60] = {.lex_state = 0, .external_lex_state = 1},
  [61] = {.lex_state = 0, .external_lex_state = 1},
  [62] = {.lex_state = 0, .external_lex_state = 1},
  [63] = {.lex_state = 0, .external_lex_state = 1},
  [64] = {.lex_state = 0, .external_lex_state = 1},
  [65] = {.lex_state = 0, .external_lex_state = 1},
  [66] = {.lex_state = 32, .external_lex_state = 1},
  [67] = {.lex_state = 32, .external_lex_state = 1},
  [68] = {.lex_state = 32, .external_lex_state = 1},
  [69] = {.lex_state = 32, .external_lex_state = 1},
  [70] = {.lex_state = 0, .external_lex_state = 1},
  [71] = {.lex_state = 32, .external_lex_state = 1},
  [72] = {.lex_state = 0, .external_lex_state = 1},
  [73] = {.lex_state = 32, .external_lex_state = 1},
  [74] = {.lex_state = 32, .external_lex_state = 1},
  [75] = {.lex_state = 0, .external_lex_state = 1},
  [76] = {.lex_state = 0, .external_lex_state = 1},
  [77] = {.lex_state = 0, .external_lex_state = 1},
  [78] = {.lex_state = 32, .external_lex_state = 1},
  [79] = {.lex_state = 32, .external_lex_state = 1},
  [80] = {.lex_state = 32, .external_lex_state = 1},
  [81] = {.lex_state = 32, .external_lex_state = 1},
  [82] = {.lex_state = 1, .external_lex_state = 1},
  [83] = {.lex_state = 1, .external_lex_state = 1},
  [84] = {.lex_state = 1, .external_lex_state = 1},
  [85] = {.lex_state = 0, .external_lex_state = 1},
  [86] = {.lex_state = 1, .external_lex_state = 1},
  [87] = {.lex_state = 1, .external_lex_state = 1},
  [88] = {.lex_state = 3, .external_lex_state = 1},
  [89] = {.lex_state = 32, .external_lex_state = 1},
  [90] = {.lex_state = 1, .external_lex_state = 1},
  [91] = {.lex_state = 1, .external_lex_state = 1},
  [92] = {.lex_state = 1, .external_lex_state = 1},
  [93] = {.lex_state = 1, .external_lex_state = 1},
  [94] = {.lex_state = 1, .external_lex_state = 1},
  [95] = {.lex_state = 1, .external_lex_state = 1},
  [96] = {.lex_state = 1, .external_lex_state = 1},
  [97] = {.lex_state = 1, .external_lex_state = 1},
  [98] = {.lex_state = 1, .external_lex_state = 1},
  [99] = {.lex_state = 1, .external_lex_state = 1},
  [100] = {.lex_state = 1, .external_lex_state = 1},
  [101] = {.lex_state = 1, .external_lex_state = 1},
  [102] = {.lex_state = 3, .external_lex_state = 1},
  [103] = {.lex_state = 1, .external_lex_state = 1},
  [104] = {.lex_state = 32, .external_lex_state = 1},
  [105] = {.lex_state = 32, .external_lex_state = 1},
  [106] = {.lex_state = 32, .external_lex_state = 1},
  [107] = {.lex_state = 32, .external_lex_state = 1},
  [108] = {.lex_state = 32, .external_lex_state = 1},
  [109] = {.lex_state = 32, .external_lex_state = 1},
  [110] = {.lex_state = 32, .external_lex_state = 1},
  [111] = {.lex_state = 32, .external_lex_state = 1},
  [112] = {.lex_state = 32, .external_lex_state = 1},
  [113] = {.lex_state = 32, .external_lex_state = 1},
  [114] = {.lex_state = 32, .external_lex_state = 1},
  [115] = {.lex_state = 1, .external_lex_state = 1},
  [116] = {.lex_state = 1, .external_lex_state = 1},
  [117] = {.lex_state = 1, .external_lex_state = 1},
  [118] = {.lex_state = 1, .external_lex_state = 1},
  [119] = {.lex_state = 1, .external_lex_state = 1},
  [120] = {.lex_state = 1, .external_lex_state = 1},
  [121] = {.lex_state = 3, .external_lex_state = 1},
  [122] = {.lex_state = 3, .external_lex_state = 1},
  [123] = {.lex_state = 0, .external_lex_state = 1},
  [124] = {.lex_state = 0, .external_lex_state = 1},
  [125] = {.lex_state = 0, .external_lex_state = 1},
  [126] = {.lex_state = 32, .external_lex_state = 1},
  [127] = {.lex_state = 1, .external_lex_state = 1},
  [128] = {.lex_state = 1, .external_lex_state = 1},
  [129] = {.lex_state = 1, .external_lex_state = 1},
  [130] = {.lex_state = 1, .external_lex_state = 1},
  [131] = {.lex_state = 32, .external_lex_state = 1},
  [132] = {.lex_state = 32, .external_lex_state = 1},
  [133] = {.lex_state = 32, .external_lex_state = 1},
  [134] = {.lex_state = 2, .external_lex_state = 1},
  [135] = {.lex_state = 2, .external_lex_state = 1},
  [136] = {.lex_state = 2, .external_lex_state = 1},
  [137] = {.lex_state = 2, .external_lex_state = 1},
  [138] = {.lex_state = 0, .external_lex_state = 1},
  [139] = {.lex_state = 2, .external_lex_state = 1},
  [140] = {.lex_state = 2, .external_lex_state = 1},
  [141] = {.lex_state = 2, .external_lex_state = 1},
  [142] = {.lex_state = 2, .external_lex_state = 1},
  [143] = {.lex_state = 2, .external_lex_state = 1},
  [144] = {.lex_state = 2, .external_lex_state = 1},
  [145] = {.lex_state = 2, .external_lex_state = 1},
  [146] = {.lex_state = 2, .external_lex_state = 1},
  [147] = {.lex_state = 2, .external_lex_state = 1},
  [148] = {.lex_state = 2, .external_lex_state = 1},
  [149] = {.lex_state = 2, .external_lex_state = 1},
  [150] = {.lex_state = 1, .external_lex_state = 1},
  [151] = {.lex_state = 32, .external_lex_state = 1},
  [152] = {.lex_state = 32, .external_lex_state = 1},
  [153] = {.lex_state = 4, .external_lex_state = 1},
  [154] = {.lex_state = 32, .external_lex_state = 1},
  [155] = {.lex_state = 4, .external_lex_state = 1},
  [156] = {.lex_state = 32, .external_lex_state = 1},
  [157] = {.lex_state = 32, .external_lex_state = 1},
  [158] = {.lex_state = 0, .external_lex_state = 1},
  [159] = {.lex_state = 4, .external_lex_state = 1},
  [160] = {.lex_state = 4, .external_lex_state = 1},
  [161] = {.lex_state = 32, .external_lex_state = 1},
  [162] = {.lex_state = 4, .external_lex_state = 1},
  [163] = {.lex_state = 2, .external_lex_state = 1},
  [164] = {.lex_state = 32, .external_lex_state = 1},
  [165] = {.lex_state = 32, .external_lex_state = 1},
  [166] = {.lex_state = 4, .external_lex_state = 1},
  [167] = {.lex_state = 2, .external_lex_state = 1},
  [168] = {.lex_state = 4, .external_lex_state = 1},
  [169] = {.lex_state = 4, .external_lex_state = 1},
  [170] = {.lex_state = 2, .external_lex_state = 1},
  [171] = {.lex_state = 2, .external_lex_state = 1},
  [172] = {.lex_state = 32, .external_lex_state = 1},
  [173] = {.lex_state = 0, .external_lex_state = 1},
  [174] = {.lex_state = 0, .external_lex_state = 1},
  [175] = {.lex_state = 0, .external_lex_state = 1},
  [176] = {.lex_state = 0, .external_lex_state = 1},
  [177] = {.lex_state = 1, .external_lex_state = 1},
  [178] = {.lex_state = 0, .external_lex_state = 1},
  [179] = {.lex_state = 0, .external_lex_state = 1},
  [180] = {.lex_state = 0, .external_lex_state = 1},
  [181] = {.lex_state = 0, .external_lex_state = 1},
  [182] = {.lex_state = 0, .external_lex_state = 1},
  [183] = {.lex_state = 0, .external_lex_state = 1},
  [184] = {.lex_state = 0, .external_lex_state = 1},
  [185] = {.lex_state = 32, .external_lex_state = 1},
  [186] = {.lex_state = 32, .external_lex_state = 1},
  [187] = {.lex_state = 32, .external_lex_state = 1},
  [188] = {.lex_state = 0, .external_lex_state = 1},
  [189] = {.lex_state = 0, .external_lex_state = 1},
  [190] = {.lex_state = 0, .external_lex_state = 1},
  [191] = {.lex_state = 32, .external_lex_state = 1},
  [192] = {.lex_state = 32, .external_lex_state = 1},
  [193] = {.lex_state = 32, .external_lex_state = 1},
  [194] = {.lex_state = 32, .external_lex_state = 1},
  [195] = {.lex_state = 32, .external_lex_state = 1},
  [196] = {.lex_state = 32, .external_lex_state = 1},
  [197] = {.lex_state = 32, .external_lex_state = 1},
  [198] = {.lex_state = 32, .external_lex_state = 1},
  [199] = {.lex_state = 32, .external_lex_state = 1},
  [200] = {.lex_state = 32, .external_lex_state = 1},
  [201] = {.lex_state = 32, .external_lex_state = 1},
  [202] = {.lex_state = 32, .external_lex_state = 1},
  [203] = {.lex_state = 32, .external_lex_state = 1},
  [204] = {.lex_state = 32, .external_lex_state = 1},
  [205] = {.lex_state = 0, .external_lex_state = 1},
  [206] = {.lex_state = 0, .external_lex_state = 1},
  [207] = {.lex_state = 0, .external_lex_state = 1},
  [208] = {.lex_state = 0, .external_lex_state = 1},
  [209] = {.lex_state = 0, .external_lex_state = 1},
  [210] = {.lex_state = 0, .external_lex_state = 1},
  [211] = {.lex_state = 0, .external_lex_state = 1},
  [212] = {.lex_state = 32, .external_lex_state = 1},
  [213] = {.lex_state = 32, .external_lex_state = 1},
  [214] = {.lex_state = 32, .external_lex_state = 1},
  [215] = {.lex_state = 32, .external_lex_state = 1},
  [216] = {.lex_state = 32, .external_lex_state = 1},
  [217] = {.lex_state = 0, .external_lex_state = 1},
  [218] = {.lex_state = 32, .external_lex_state = 1},
  [219] = {.lex_state = 32, .external_lex_state = 1},
  [220] = {.lex_state = 0, .external_lex_state = 1},
  [221] = {.lex_state = 0, .external_lex_state = 1},
  [222] = {.lex_state = 32, .external_lex_state = 1},
  [223] = {.lex_state = 0, .external_lex_state = 1},
  [224] = {.lex_state = 0, .external_lex_state = 1},
  [225] = {.lex_state = 0, .external_lex_state = 1},
  [226] = {.lex_state = 0, .external_lex_state = 1},
  [227] = {.lex_state = 0, .external_lex_state = 1},
  [228] = {.lex_state = 32, .external_lex_state = 1},
  [229] = {.lex_state = 32, .external_lex_state = 1},
  [230] = {.lex_state = 1, .external_lex_state = 1},
  [231] = {.lex_state = 0, .external_lex_state = 1},
  [232] = {.lex_state = 0, .external_lex_state = 1},
  [233] = {.lex_state = 0, .external_lex_state = 1},
  [234] = {.lex_state = 0, .external_lex_state = 1},
  [235] = {.lex_state = 32, .external_lex_state = 1},
  [236] = {.lex_state = 0, .external_lex_state = 1},
  [237] = {.lex_state = 0, .external_lex_state = 1},
  [238] = {.lex_state = 0, .external_lex_state = 1},
  [239] = {.lex_state = 1, .external_lex_state = 1},
  [240] = {.lex_state = 0, .external_lex_state = 1},
  [241] = {.lex_state = 32, .external_lex_state = 1},
  [242] = {.lex_state = 0, .external_lex_state = 1},
  [243] = {.lex_state = 0, .external_lex_state = 1},
  [244] = {.lex_state = 0, .external_lex_state = 1},
  [245] = {.lex_state = 32, .external_lex_state = 1},
  [246] = {.lex_state = 0, .external_lex_state = 1},
  [247] = {.lex_state = 32, .external_lex_state = 1},
  [248] = {.lex_state = 0, .external_lex_state = 1},
  [249] = {.lex_state = 0, .external_lex_state = 1},
  [250] = {.lex_state = 0, .external_lex_state = 1},
  [251] = {.lex_state = 32, .external_lex_state = 1},
  [252] = {.lex_state = 0, .external_lex_state = 1},
  [253] = {.lex_state = 0, .external_lex_state = 1},
  [254] = {.lex_state = 32, .external_lex_state = 1},
  [255] = {.lex_state = 0, .external_lex_state = 1},
  [256] = {.lex_state = 0, .external_lex_state = 1},
  [257] = {.lex_state = 0, .external_lex_state = 1},
  [258] = {.lex_state = 0, .external_lex_state = 1},
  [259] = {.lex_state = 0, .external_lex_state = 1},
  [260] = {.lex_state = 32, .external_lex_state = 1},
  [261] = {.lex_state = 0, .external_lex_state = 1},
  [262] = {.lex_state = 32, .external_lex_state = 1},
  [263] = {.lex_state = 0, .external_lex_state = 1},
  [264] = {.lex_state = 0, .external_lex_state = 1},
  [265] = {.lex_state = 0, .external_lex_state = 1},
  [266] = {.lex_state = 0, .external_lex_state = 1},
  [267] = {.lex_state = 0, .external_lex_state = 1},
  [268] = {.lex_state = 0, .external_lex_state = 1},
  [269] = {.lex_state = 0, .external_lex_state = 1},
  [270] = {.lex_state = 0, .external_lex_state = 1},
  [271] = {.lex_state = 32, .external_lex_state = 1},
  [272] = {.lex_state = 0, .external_lex_state = 1},
  [273] = {.lex_state = 0, .external_lex_state = 1},
  [274] = {.lex_state = 32, .external_lex_state = 1},
  [275] = {.lex_state = 0, .external_lex_state = 1},
  [276] = {.lex_state = 0, .external_lex_state = 1},
  [277] = {.lex_state = 0, .external_lex_state = 1},
  [278] = {.lex_state = 0, .external_lex_state = 1},
  [279] = {.lex_state = 3, .external_lex_state = 1},
  [280] = {.lex_state = 0, .external_lex_state = 1},
  [281] = {.lex_state = 0, .external_lex_state = 1},
  [282] = {.lex_state = 0, .external_lex_state = 1},
  [283] = {.lex_state = 0, .external_lex_state = 1},
  [284] = {.lex_state = 0, .external_lex_state = 1},
  [285] = {.lex_state = 0, .external_lex_state = 1},
  [286] = {.lex_state = 0, .external_lex_state = 1},
  [287] = {.lex_state = 0, .external_lex_state = 1},
  [288] = {.lex_state = 0, .external_lex_state = 1},
  [289] = {.lex_state = 32, .external_lex_state = 1},
  [290] = {.lex_state = 32, .external_lex_state = 1},
  [291] = {.lex_state = 32, .external_lex_state = 1},
  [292] = {.lex_state = 0, .external_lex_state = 1},
  [293] = {.lex_state = 0, .external_lex_state = 1},
  [294] = {.lex_state = 0, .external_lex_state = 1},
  [295] = {.lex_state = 0, .external_lex_state = 1},
  [296] = {.lex_state = 32, .external_lex_state = 1},
  [297] = {.lex_state = 32, .external_lex_state = 1},
  [298] = {.lex_state = 0, .external_lex_state = 1},
  [299] = {.lex_state = 32, .external_lex_state = 1},
  [300] = {.lex_state = 0, .external_lex_state = 1},
  [301] = {.lex_state = 0, .external_lex_state = 1},
  [302] = {.lex_state = 0, .external_lex_state = 1},
  [303] = {.lex_state = 0, .external_lex_state = 1},
  [304] = {.lex_state = 0, .external_lex_state = 1},
  [305] = {.lex_state = 0, .external_lex_state = 1},
  [306] = {.lex_state = 32, .external_lex_state = 1},
  [307] = {.lex_state = 0, .external_lex_state = 1},
  [308] = {.lex_state = 0, .external_lex_state = 1},
  [309] = {.lex_state = 0, .external_lex_state = 1},
  [310] = {.lex_state = 32, .external_lex_state = 1},
  [311] = {.lex_state = 32, .external_lex_state = 1},
  [312] = {.lex_state = 0, .external_lex_state = 1},
  [313] = {.lex_state = 3, .external_lex_state = 1},
  [314] = {.lex_state = 0, .external_lex_state = 1},
  [315] = {.lex_state = 0, .external_lex_state = 1},
  [316] = {.lex_state = 0, .external_lex_state = 1},
  [317] = {.lex_state = 0, .external_lex_state = 1},
  [318] = {.lex_state = 32, .external_lex_state = 1},
  [319] = {.lex_state = 0, .external_lex_state = 1},
  [320] = {.lex_state = 0, .external_lex_state = 1},
  [321] = {.lex_state = 119, .external_lex_state = 1},
  [322] = {.lex_state = 0, .external_lex_state = 1},
  [323] = {.lex_state = 0, .external_lex_state = 1},
  [324] = {.lex_state = 0, .external_lex_state = 1},
  [325] = {.lex_state = 0, .external_lex_state = 1},
  [326] = {.lex_state = 0, .external_lex_state = 1},
  [327] = {.lex_state = 0, .external_lex_state = 1},
  [328] = {.lex_state = 0, .external_lex_state = 1},
  [329] = {.lex_state = 0, .external_lex_state = 1},
  [330] = {.lex_state = 32, .external_lex_state = 1},
  [331] = {.lex_state = 0, .external_lex_state = 1},
  [332] = {.lex_state = 0, .external_lex_state = 1},
  [333] = {.lex_state = 0, .external_lex_state = 1},
  [334] = {.lex_state = 0, .external_lex_state = 1},
  [335] = {.lex_state = 0, .external_lex_state = 1},
  [336] = {.lex_state = 0, .external_lex_state = 1},
  [337] = {.lex_state = 32, .external_lex_state = 1},
  [338] = {.lex_state = 0, .external_lex_state = 1},
  [339] = {.lex_state = 0, .external_lex_state = 1},
  [340] = {.lex_state = 0, .external_lex_state = 1},
  [341] = {.lex_state = 0, .external_lex_state = 1},
  [342] = {.lex_state = 0, .external_lex_state = 1},
  [343] = {.lex_state = 0, .external_lex_state = 1},
  [344] = {.lex_state = 0, .external_lex_state = 1},
  [345] = {.lex_state = 0, .external_lex_state = 1},
  [346] = {.lex_state = 0, .external_lex_state = 1},
  [347] = {.lex_state = 0, .external_lex_state = 1},
  [348] = {.lex_state = 0, .external_lex_state = 1},
  [349] = {.lex_state = 0, .external_lex_state = 1},
  [350] = {.lex_state = 0, .external_lex_state = 1},
  [351] = {.lex_state = 0, .external_lex_state = 1},
  [352] = {.lex_state = 0, .external_lex_state = 1},
  [353] = {.lex_state = 0, .external_lex_state = 1},
  [354] = {.lex_state = 0, .external_lex_state = 1},
  [355] = {.lex_state = 0, .external_lex_state = 1},
  [356] = {.lex_state = 0, .external_lex_state = 1},
  [357] = {.lex_state = 0, .external_lex_state = 1},
  [358] = {.lex_state = 0, .external_lex_state = 1},
  [359] = {.lex_state = 32, .external_lex_state = 1},
  [360] = {.lex_state = 32, .external_lex_state = 1},
  [361] = {.lex_state = 0, .external_lex_state = 1},
  [362] = {.lex_state = 0, .external_lex_state = 1},
  [363] = {.lex_state = 0, .external_lex_state = 1},
  [364] = {.lex_state = 0, .external_lex_state = 1},
  [365] = {.lex_state = 0, .external_lex_state = 1},
  [366] = {.lex_state = 0, .external_lex_state = 1},
  [367] = {.lex_state = 32, .external_lex_state = 1},
  [368] = {.lex_state = 0, .external_lex_state = 1},
  [369] = {.lex_state = 0, .external_lex_state = 1},
  [370] = {.lex_state = 0, .external_lex_state = 1},
  [371] = {.lex_state = 32, .external_lex_state = 1},
  [372] = {.lex_state = 32, .external_lex_state = 1},
  [373] = {.lex_state = 32, .external_lex_state = 1},
  [374] = {.lex_state = 0, .external_lex_state = 1},
  [375] = {.lex_state = 0, .external_lex_state = 1},
  [376] = {.lex_state = 32, .external_lex_state = 1},
  [377] = {.lex_state = 0, .external_lex_state = 1},
  [378] = {.lex_state = 0, .external_lex_state = 1},
  [379] = {.lex_state = 0, .external_lex_state = 1},
  [380] = {.lex_state = 0, .external_lex_state = 1},
  [381] = {.lex_state = 0, .external_lex_state = 1},
  [382] = {.lex_state = 0, .external_lex_state = 1},
  [383] = {.lex_state = 0, .external_lex_state = 1},
  [384] = {.lex_state = 0, .external_lex_state = 1},
  [385] = {.lex_state = 32, .external_lex_state = 1},
  [386] = {.lex_state = 0, .external_lex_state = 1},
  [387] = {.lex_state = 0, .external_lex_state = 1},
  [388] = {.lex_state = 0, .external_lex_state = 1},
  [389] = {.lex_state = 0, .external_lex_state = 1},
  [390] = {.lex_state = 0, .external_lex_state = 1},
  [391] = {.lex_state = 0, .external_lex_state = 1},
  [392] = {.lex_state = 0, .external_lex_state = 1},
  [393] = {.lex_state = 32, .external_lex_state = 1},
  [394] = {.lex_state = 32, .external_lex_state = 1},
  [395] = {.lex_state = 0, .external_lex_state = 1},
  [396] = {.lex_state = 32, .external_lex_state = 1},
  [397] = {.lex_state = 32, .external_lex_state = 1},
  [398] = {.lex_state = 0, .external_lex_state = 1},
  [399] = {.lex_state = 0, .external_lex_state = 1},
  [400] = {.lex_state = 32, .external_lex_state = 1},
  [401] = {.lex_state = 0, .external_lex_state = 1},
  [402] = {.lex_state = 0, .external_lex_state = 1},
  [403] = {.lex_state = 32, .external_lex_state = 1},
  [404] = {.lex_state = 32, .external_lex_state = 1},
  [405] = {.lex_state = 0, .external_lex_state = 1},
  [406] = {.lex_state = 0, .external_lex_state = 1},
  [407] = {.lex_state = 0, .external_lex_state = 1},
  [408] = {.lex_state = 0, .external_lex_state = 1},
  [409] = {.lex_state = 0, .external_lex_state = 1},
  [410] = {.lex_state = 0, .external_lex_state = 1},
  [411] = {.lex_state = 0, .external_lex_state = 1},
  [412] = {.lex_state = 0, .external_lex_state = 1},
  [413] = {.lex_state = 0, .external_lex_state = 1},
  [414] = {.lex_state = 32, .external_lex_state = 1},
  [415] = {.lex_state = 0, .external_lex_state = 1},
  [416] = {.lex_state = 0, .external_lex_state = 1},
  [417] = {.lex_state = 0, .external_lex_state = 1},
  [418] = {.lex_state = 0, .external_lex_state = 1},
  [419] = {.lex_state = 0, .external_lex_state = 1},
  [420] = {.lex_state = 0, .external_lex_state = 1},
  [421] = {(TSStateId)(-1)},
};

enum {
  ts_external_token__block_comment = 0,
};

static const TSSymbol ts_external_scanner_symbol_map[EXTERNAL_TOKEN_COUNT] = {
  [ts_external_token__block_comment] = sym__block_comment,
};

static const bool ts_external_scanner_states[2][EXTERNAL_TOKEN_COUNT] = {
  [1] = {
    [ts_external_token__block_comment] = true,
  },
};

static const uint16_t ts_parse_table[LARGE_STATE_COUNT][SYMBOL_COUNT] = {
  [0] = {
    [sym__comment] = STATE(0),
    [ts_builtin_sym_end] = ACTIONS(1),
    [sym_ident] = ACTIONS(1),
    [aux_sym_int_literal_token1] = ACTIONS(1),
    [aux_sym_int_literal_token2] = ACTIONS(1),
    [aux_sym_int_literal_token3] = ACTIONS(1),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(1),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(1),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(1),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(1),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(1),
    [aux_sym_hex_float_literal_token1] = ACTIONS(1),
    [aux_sym_hex_float_literal_token2] = ACTIONS(1),
    [aux_sym_hex_float_literal_token3] = ACTIONS(1),
    [anon_sym_read] = ACTIONS(1),
    [anon_sym_write] = ACTIONS(1),
    [anon_sym_read_write] = ACTIONS(1),
    [anon_sym_rgba8unorm] = ACTIONS(1),
    [anon_sym_rgba8snorm] = ACTIONS(1),
    [anon_sym_rgba8uint] = ACTIONS(1),
    [anon_sym_rgba8sint] = ACTIONS(1),
    [anon_sym_rgba16uint] = ACTIONS(1),
    [anon_sym_rgba16sint] = ACTIONS(1),
    [anon_sym_rgba16float] = ACTIONS(1),
    [anon_sym_r32uint] = ACTIONS(1),
    [anon_sym_r32sint] = ACTIONS(1),
    [anon_sym_r32float] = ACTIONS(1),
    [anon_sym_rg32uint] = ACTIONS(1),
    [anon_sym_rg32sint] = ACTIONS(1),
    [anon_sym_rg32float] = ACTIONS(1),
    [anon_sym_rgba32uint] = ACTIONS(1),
    [anon_sym_rgba32sint] = ACTIONS(1),
    [anon_sym_rgba32float] = ACTIONS(1),
    [sym_array] = ACTIONS(1),
    [sym_atomic] = ACTIONS(1),
    [sym_bool] = ACTIONS(1),
    [sym_float32] = ACTIONS(1),
    [sym_float16] = ACTIONS(1),
    [sym_int32] = ACTIONS(1),
    [sym_mat2x2] = ACTIONS(1),
    [sym_mat2x3] = ACTIONS(1),
    [sym_mat2x4] = ACTIONS(1),
    [sym_mat3x2] = ACTIONS(1),
    [sym_mat3x3] = ACTIONS(1),
    [sym_mat3x4] = ACTIONS(1),
    [sym_mat4x2] = ACTIONS(1),
    [sym_mat4x3] = ACTIONS(1),
    [sym_mat4x4] = ACTIONS(1),
    [sym_override] = ACTIONS(1),
    [sym_pointer] = ACTIONS(1),
    [sym_sampler] = ACTIONS(1),
    [sym_sampler_comparison] = ACTIONS(1),
    [sym_staticAssert] = ACTIONS(1),
    [sym_struct] = ACTIONS(1),
    [sym_texture_1d] = ACTIONS(1),
    [sym_texture_2d] = ACTIONS(1),
    [sym_texture_2d_array] = ACTIONS(1),
    [sym_texture_3d] = ACTIONS(1),
    [sym_texture_cube] = ACTIONS(1),
    [sym_texture_cube_array] = ACTIONS(1),
    [sym_texture_multisampled_2d] = ACTIONS(1),
    [sym_texture_storage_1d] = ACTIONS(1),
    [sym_texture_storage_2d] = ACTIONS(1),
    [sym_texture_storage_2d_array] = ACTIONS(1),
    [sym_texture_storage_3d] = ACTIONS(1),
    [sym_texture_depth_2d] = ACTIONS(1),
    [sym_texture_depth_2d_array] = ACTIONS(1),
    [sym_texture_depth_cube] = ACTIONS(1),
    [sym_texture_depth_cube_array] = ACTIONS(1),
    [sym_texture_depth_multisampled_2d] = ACTIONS(1),
    [sym_uint32] = ACTIONS(1),
    [sym_vec2] = ACTIONS(1),
    [sym_vec3] = ACTIONS(1),
    [sym_vec4] = ACTIONS(1),
    [sym_bitcast] = ACTIONS(1),
    [sym_break] = ACTIONS(1),
    [sym_case] = ACTIONS(1),
    [sym_const] = ACTIONS(1),
    [sym_continue] = ACTIONS(1),
    [sym_continuing] = ACTIONS(1),
    [sym_default] = ACTIONS(1),
    [sym_discard] = ACTIONS(1),
    [sym_else] = ACTIONS(1),
    [sym_enable] = ACTIONS(1),
    [sym_fallthrough] = ACTIONS(1),
    [sym_false] = ACTIONS(1),
    [sym_fn] = ACTIONS(1),
    [sym_for] = ACTIONS(1),
    [sym_function] = ACTIONS(1),
    [sym_if] = ACTIONS(1),
    [sym_let] = ACTIONS(1),
    [sym_loop] = ACTIONS(1),
    [sym_private] = ACTIONS(1),
    [sym_return] = ACTIONS(1),
    [sym_storage] = ACTIONS(1),
    [sym_switch] = ACTIONS(1),
    [sym_true] = ACTIONS(1),
    [sym_type] = ACTIONS(1),
    [sym_uniform] = ACTIONS(1),
    [sym_var] = ACTIONS(1),
    [sym_while] = ACTIONS(1),
    [sym_workgroup] = ACTIONS(1),
    [sym_and] = ACTIONS(1),
    [sym_and_and] = ACTIONS(1),
    [sym_arrow] = ACTIONS(1),
    [sym_attr] = ACTIONS(1),
    [sym_forward_slash] = ACTIONS(1),
    [sym_bang] = ACTIONS(1),
    [sym_bracket_left] = ACTIONS(1),
    [sym_bracket_right] = ACTIONS(1),
    [sym_brace_left] = ACTIONS(1),
    [sym_brace_right] = ACTIONS(1),
    [sym_colon] = ACTIONS(1),
    [sym_comma] = ACTIONS(1),
    [sym_equal] = ACTIONS(1),
    [sym_equal_equal] = ACTIONS(1),
    [sym_not_equal] = ACTIONS(1),
    [sym_greater_than] = ACTIONS(1),
    [sym_greater_than_equal] = ACTIONS(1),
    [sym_less_than] = ACTIONS(1),
    [sym_less_than_equal] = ACTIONS(1),
    [sym_modulo] = ACTIONS(1),
    [sym_minus] = ACTIONS(1),
    [sym_period] = ACTIONS(1),
    [sym_plus] = ACTIONS(1),
    [sym_plus_plus] = ACTIONS(1),
    [sym_or] = ACTIONS(1),
    [sym_or_or] = ACTIONS(1),
    [sym_paren_left] = ACTIONS(1),
    [sym_paren_right] = ACTIONS(1),
    [sym_semicolon] = ACTIONS(1),
    [sym_star] = ACTIONS(1),
    [sym_tilde] = ACTIONS(1),
    [sym_underscore] = ACTIONS(1),
    [sym_xor] = ACTIONS(1),
    [sym_plus_equal] = ACTIONS(1),
    [sym_minus_equal] = ACTIONS(1),
    [sym_times_equal] = ACTIONS(1),
    [sym_division_equal] = ACTIONS(1),
    [sym_modulo_equal] = ACTIONS(1),
    [sym_and_equal] = ACTIONS(1),
    [sym_or_equal] = ACTIONS(1),
    [sym_xor_equal] = ACTIONS(1),
    [anon_sym_AppendStructuredBuffer] = ACTIONS(1),
    [anon_sym_BlendState] = ACTIONS(1),
    [anon_sym_Buffer] = ACTIONS(1),
    [anon_sym_ByteAddressBuffer] = ACTIONS(1),
    [anon_sym_CompileShader] = ACTIONS(1),
    [anon_sym_ComputeShader] = ACTIONS(1),
    [anon_sym_ConsumeStructuredBuffer] = ACTIONS(1),
    [anon_sym_DepthStencilState] = ACTIONS(1),
    [anon_sym_DepthStencilView] = ACTIONS(1),
    [anon_sym_DomainShader] = ACTIONS(1),
    [anon_sym_GeometryShader] = ACTIONS(1),
    [anon_sym_Hullshader] = ACTIONS(1),
    [anon_sym_InputPatch] = ACTIONS(1),
    [anon_sym_LineStream] = ACTIONS(1),
    [anon_sym_NULL] = ACTIONS(1),
    [anon_sym_OutputPatch] = ACTIONS(1),
    [anon_sym_PixelShader] = ACTIONS(1),
    [anon_sym_PointStream] = ACTIONS(1),
    [anon_sym_RWBuffer] = ACTIONS(1),
    [anon_sym_RWByteAddressBuffer] = ACTIONS(1),
    [anon_sym_RWStructuredBuffer] = ACTIONS(1),
    [anon_sym_RWTexture1D] = ACTIONS(1),
    [anon_sym_RWTexture1DArray] = ACTIONS(1),
    [anon_sym_RWTexture2D] = ACTIONS(1),
    [anon_sym_RWTexture2DArray] = ACTIONS(1),
    [anon_sym_RWTexture3D] = ACTIONS(1),
    [anon_sym_RasterizerState] = ACTIONS(1),
    [anon_sym_RenderTargetView] = ACTIONS(1),
    [anon_sym_SamplerComparisonState] = ACTIONS(1),
    [anon_sym_SamplerState] = ACTIONS(1),
    [anon_sym_Self] = ACTIONS(1),
    [anon_sym_StructuredBuffer] = ACTIONS(1),
    [anon_sym_Texture1D] = ACTIONS(1),
    [anon_sym_Texture1DArray] = ACTIONS(1),
    [anon_sym_Texture2D] = ACTIONS(1),
    [anon_sym_Texture2DArray] = ACTIONS(1),
    [anon_sym_Texture2DMS] = ACTIONS(1),
    [anon_sym_Texture2DMSArray] = ACTIONS(1),
    [anon_sym_Texture3D] = ACTIONS(1),
    [anon_sym_TextureCube] = ACTIONS(1),
    [anon_sym_TextureCubeArray] = ACTIONS(1),
    [anon_sym_TriangleStream] = ACTIONS(1),
    [anon_sym_VertexShader] = ACTIONS(1),
    [anon_sym_abstract] = ACTIONS(1),
    [anon_sym_active] = ACTIONS(1),
    [anon_sym_alignas] = ACTIONS(1),
    [anon_sym_alignof] = ACTIONS(1),
    [anon_sym_as] = ACTIONS(1),
    [anon_sym_asm] = ACTIONS(1),
    [anon_sym_asm_fragment] = ACTIONS(1),
    [anon_sym_async] = ACTIONS(1),
    [anon_sym_atomic_uint] = ACTIONS(1),
    [anon_sym_attribute] = ACTIONS(1),
    [anon_sym_auto] = ACTIONS(1),
    [anon_sym_await] = ACTIONS(1),
    [anon_sym_become] = ACTIONS(1),
    [anon_sym_bf16] = ACTIONS(1),
    [anon_sym_binding_array] = ACTIONS(1),
    [anon_sym_cast] = ACTIONS(1),
    [anon_sym_catch] = ACTIONS(1),
    [anon_sym_cbuffer] = ACTIONS(1),
    [anon_sym_centroid] = ACTIONS(1),
    [anon_sym_char] = ACTIONS(1),
    [anon_sym_class] = ACTIONS(1),
    [anon_sym_co_await] = ACTIONS(1),
    [anon_sym_co_return] = ACTIONS(1),
    [anon_sym_co_yield] = ACTIONS(1),
    [anon_sym_coherent] = ACTIONS(1),
    [anon_sym_column_major] = ACTIONS(1),
    [anon_sym_common] = ACTIONS(1),
    [anon_sym_compile] = ACTIONS(1),
    [anon_sym_compile_fragment] = ACTIONS(1),
    [anon_sym_concept] = ACTIONS(1),
    [anon_sym_const_cast] = ACTIONS(1),
    [anon_sym_consteval] = ACTIONS(1),
    [anon_sym_constexpr] = ACTIONS(1),
    [anon_sym_constinit] = ACTIONS(1),
    [anon_sym_crate] = ACTIONS(1),
    [anon_sym_debugger] = ACTIONS(1),
    [anon_sym_decltype] = ACTIONS(1),
    [anon_sym_delete] = ACTIONS(1),
    [anon_sym_demote] = ACTIONS(1),
    [anon_sym_demote_to_helper] = ACTIONS(1),
    [anon_sym_do] = ACTIONS(1),
    [anon_sym_dword] = ACTIONS(1),
    [anon_sym_dynamic_cast] = ACTIONS(1),
    [anon_sym_enum] = ACTIONS(1),
    [anon_sym_explicit] = ACTIONS(1),
    [anon_sym_export] = ACTIONS(1),
    [anon_sym_extends] = ACTIONS(1),
    [anon_sym_extern] = ACTIONS(1),
    [anon_sym_external] = ACTIONS(1),
    [anon_sym_f64] = ACTIONS(1),
    [anon_sym_filter] = ACTIONS(1),
    [anon_sym_final] = ACTIONS(1),
    [anon_sym_finally] = ACTIONS(1),
    [anon_sym_fixed] = ACTIONS(1),
    [anon_sym_flat] = ACTIONS(1),
    [anon_sym_friend] = ACTIONS(1),
    [anon_sym_from] = ACTIONS(1),
    [anon_sym_fvec2] = ACTIONS(1),
    [anon_sym_fvec3] = ACTIONS(1),
    [anon_sym_fvec4] = ACTIONS(1),
    [anon_sym_fxgroup] = ACTIONS(1),
    [anon_sym_get] = ACTIONS(1),
    [anon_sym_goto] = ACTIONS(1),
    [anon_sym_groupshared] = ACTIONS(1),
    [anon_sym_handle] = ACTIONS(1),
    [anon_sym_highp] = ACTIONS(1),
    [anon_sym_hvec2] = ACTIONS(1),
    [anon_sym_hvec3] = ACTIONS(1),
    [anon_sym_hvec4] = ACTIONS(1),
    [anon_sym_i16] = ACTIONS(1),
    [anon_sym_i64] = ACTIONS(1),
    [anon_sym_i8] = ACTIONS(1),
    [anon_sym_iimage1D] = ACTIONS(1),
    [anon_sym_iimage1DArray] = ACTIONS(1),
    [anon_sym_iimage2D] = ACTIONS(1),
    [anon_sym_iimage2DArray] = ACTIONS(1),
    [anon_sym_iimage2DMS] = ACTIONS(1),
    [anon_sym_iimage2DMSArray] = ACTIONS(1),
    [anon_sym_iimage2DRect] = ACTIONS(1),
    [anon_sym_iimage3D] = ACTIONS(1),
    [anon_sym_iimageBuffer] = ACTIONS(1),
    [anon_sym_iimageCube] = ACTIONS(1),
    [anon_sym_iimageCubeArray] = ACTIONS(1),
    [anon_sym_image1D] = ACTIONS(1),
    [anon_sym_image1DArray] = ACTIONS(1),
    [anon_sym_image2D] = ACTIONS(1),
    [anon_sym_image2DArray] = ACTIONS(1),
    [anon_sym_image2DMS] = ACTIONS(1),
    [anon_sym_image2DMSArray] = ACTIONS(1),
    [anon_sym_image2DRect] = ACTIONS(1),
    [anon_sym_image3D] = ACTIONS(1),
    [anon_sym_imageBuffer] = ACTIONS(1),
    [anon_sym_imageCube] = ACTIONS(1),
    [anon_sym_imageCubeArray] = ACTIONS(1),
    [anon_sym_impl] = ACTIONS(1),
    [anon_sym_implements] = ACTIONS(1),
    [anon_sym_import] = ACTIONS(1),
    [anon_sym_inline] = ACTIONS(1),
    [anon_sym_inout] = ACTIONS(1),
    [anon_sym_instanceof] = ACTIONS(1),
    [anon_sym_interface] = ACTIONS(1),
    [anon_sym_invariant] = ACTIONS(1),
    [anon_sym_isampler1D] = ACTIONS(1),
    [anon_sym_isampler1DArray] = ACTIONS(1),
    [anon_sym_isampler2D] = ACTIONS(1),
    [anon_sym_isampler2DArray] = ACTIONS(1),
    [anon_sym_isampler2DMS] = ACTIONS(1),
    [anon_sym_isampler2DMSArray] = ACTIONS(1),
    [anon_sym_isampler2DRect] = ACTIONS(1),
    [anon_sym_isampler3D] = ACTIONS(1),
    [anon_sym_isamplerBuffer] = ACTIONS(1),
    [anon_sym_isamplerCube] = ACTIONS(1),
    [anon_sym_isamplerCubeArray] = ACTIONS(1),
    [anon_sym_isubpassInput] = ACTIONS(1),
    [anon_sym_isubpassInputMS] = ACTIONS(1),
    [anon_sym_itexture1D] = ACTIONS(1),
    [anon_sym_itexture1DArray] = ACTIONS(1),
    [anon_sym_itexture2D] = ACTIONS(1),
    [anon_sym_itexture2DArray] = ACTIONS(1),
    [anon_sym_itexture2DMS] = ACTIONS(1),
    [anon_sym_itexture2DMSArray] = ACTIONS(1),
    [anon_sym_itexture2DRect] = ACTIONS(1),
    [anon_sym_itexture3D] = ACTIONS(1),
    [anon_sym_itextureBuffer] = ACTIONS(1),
    [anon_sym_itextureCube] = ACTIONS(1),
    [anon_sym_itextureCubeArray] = ACTIONS(1),
    [anon_sym_layout] = ACTIONS(1),
    [anon_sym_line] = ACTIONS(1),
    [anon_sym_lineadj] = ACTIONS(1),
    [anon_sym_linear] = ACTIONS(1),
    [anon_sym_lowp] = ACTIONS(1),
    [anon_sym_macro] = ACTIONS(1),
    [anon_sym_macro_rules] = ACTIONS(1),
    [anon_sym_mat] = ACTIONS(1),
    [anon_sym_match] = ACTIONS(1),
    [anon_sym_matrix] = ACTIONS(1),
    [anon_sym_mediump] = ACTIONS(1),
    [anon_sym_meta] = ACTIONS(1),
    [anon_sym_mod] = ACTIONS(1),
    [anon_sym_module] = ACTIONS(1),
    [anon_sym_move] = ACTIONS(1),
    [anon_sym_mut] = ACTIONS(1),
    [anon_sym_mutable] = ACTIONS(1),
    [anon_sym_namespace] = ACTIONS(1),
    [anon_sym_new] = ACTIONS(1),
    [anon_sym_nil] = ACTIONS(1),
    [anon_sym_noexcept] = ACTIONS(1),
    [anon_sym_noinline] = ACTIONS(1),
    [anon_sym_nointerpolation] = ACTIONS(1),
    [anon_sym_noperspective] = ACTIONS(1),
    [anon_sym_null] = ACTIONS(1),
    [anon_sym_nullptr] = ACTIONS(1),
    [anon_sym_of] = ACTIONS(1),
    [anon_sym_operator] = ACTIONS(1),
    [anon_sym_package] = ACTIONS(1),
    [anon_sym_packoffset] = ACTIONS(1),
    [anon_sym_partition] = ACTIONS(1),
    [anon_sym_pass] = ACTIONS(1),
    [anon_sym_patch] = ACTIONS(1),
    [anon_sym_pixelfragment] = ACTIONS(1),
    [anon_sym_point] = ACTIONS(1),
    [anon_sym_precise] = ACTIONS(1),
    [anon_sym_precision] = ACTIONS(1),
    [anon_sym_premerge] = ACTIONS(1),
    [anon_sym_priv] = ACTIONS(1),
    [anon_sym_protected] = ACTIONS(1),
    [anon_sym_pub] = ACTIONS(1),
    [anon_sym_public] = ACTIONS(1),
    [anon_sym_readonly] = ACTIONS(1),
    [anon_sym_ref] = ACTIONS(1),
    [anon_sym_regardless] = ACTIONS(1),
    [anon_sym_register] = ACTIONS(1),
    [anon_sym_reinterpret_cast] = ACTIONS(1),
    [anon_sym_requires] = ACTIONS(1),
    [anon_sym_resource] = ACTIONS(1),
    [anon_sym_restrict] = ACTIONS(1),
    [anon_sym_row_major] = ACTIONS(1),
    [anon_sym_samper] = ACTIONS(1),
    [anon_sym_sample] = ACTIONS(1),
    [anon_sym_sampler1D] = ACTIONS(1),
    [anon_sym_sampler1DArray] = ACTIONS(1),
    [anon_sym_sampler1DArrayShadow] = ACTIONS(1),
    [anon_sym_sampler1DShadow] = ACTIONS(1),
    [anon_sym_sampler2D] = ACTIONS(1),
    [anon_sym_sampler2DArray] = ACTIONS(1),
    [anon_sym_sampler2DArrayShadow] = ACTIONS(1),
    [anon_sym_sampler2DMS] = ACTIONS(1),
    [anon_sym_sampler2DMSArray] = ACTIONS(1),
    [anon_sym_sampler2DRect] = ACTIONS(1),
    [anon_sym_sampler2DRectShadow] = ACTIONS(1),
    [anon_sym_sampler2DShadow] = ACTIONS(1),
    [anon_sym_sampler3D] = ACTIONS(1),
    [anon_sym_sampler3DRect] = ACTIONS(1),
    [anon_sym_samplerBuffer] = ACTIONS(1),
    [anon_sym_samplerCube] = ACTIONS(1),
    [anon_sym_samplerCubeArray] = ACTIONS(1),
    [anon_sym_samplerCubeArrayShadow] = ACTIONS(1),
    [anon_sym_samplerCubeShadow] = ACTIONS(1),
    [anon_sym_samplerShadow] = ACTIONS(1),
    [anon_sym_self] = ACTIONS(1),
    [anon_sym_set] = ACTIONS(1),
    [anon_sym_shared] = ACTIONS(1),
    [anon_sym_signed] = ACTIONS(1),
    [anon_sym_sizeof] = ACTIONS(1),
    [anon_sym_smooth] = ACTIONS(1),
    [anon_sym_snorm] = ACTIONS(1),
    [anon_sym_stateblock] = ACTIONS(1),
    [anon_sym_stateblock_state] = ACTIONS(1),
    [anon_sym_static] = ACTIONS(1),
    [anon_sym_static_assert] = ACTIONS(1),
    [anon_sym_static_cast] = ACTIONS(1),
    [anon_sym_std] = ACTIONS(1),
    [anon_sym_string] = ACTIONS(1),
    [anon_sym_subpassInput] = ACTIONS(1),
    [anon_sym_subpassInputMS] = ACTIONS(1),
    [anon_sym_subroutine] = ACTIONS(1),
    [anon_sym_super] = ACTIONS(1),
    [anon_sym_superp] = ACTIONS(1),
    [anon_sym_target] = ACTIONS(1),
    [anon_sym_tbuffer] = ACTIONS(1),
    [anon_sym_technique] = ACTIONS(1),
    [anon_sym_technique10] = ACTIONS(1),
    [anon_sym_technique11] = ACTIONS(1),
    [anon_sym_template] = ACTIONS(1),
    [anon_sym_texture1D] = ACTIONS(1),
    [anon_sym_texture1DArray] = ACTIONS(1),
    [anon_sym_texture2D] = ACTIONS(1),
    [anon_sym_texture2DArray] = ACTIONS(1),
    [anon_sym_texture2DMS] = ACTIONS(1),
    [anon_sym_texture2DMSArray] = ACTIONS(1),
    [anon_sym_texture2DRect] = ACTIONS(1),
    [anon_sym_texture3D] = ACTIONS(1),
    [anon_sym_textureBuffer] = ACTIONS(1),
    [anon_sym_textureCube] = ACTIONS(1),
    [anon_sym_textureCubeArray] = ACTIONS(1),
    [anon_sym_this] = ACTIONS(1),
    [anon_sym_thread_local] = ACTIONS(1),
    [anon_sym_throw] = ACTIONS(1),
    [anon_sym_trait] = ACTIONS(1),
    [anon_sym_triangle] = ACTIONS(1),
    [anon_sym_triangleadj] = ACTIONS(1),
    [anon_sym_try] = ACTIONS(1),
    [anon_sym_typedef] = ACTIONS(1),
    [anon_sym_typeid] = ACTIONS(1),
    [anon_sym_typename] = ACTIONS(1),
    [anon_sym_typeof] = ACTIONS(1),
    [anon_sym_u16] = ACTIONS(1),
    [anon_sym_u64] = ACTIONS(1),
    [anon_sym_u8] = ACTIONS(1),
    [anon_sym_uimage1D] = ACTIONS(1),
    [anon_sym_uimage1DArray] = ACTIONS(1),
    [anon_sym_uimage2D] = ACTIONS(1),
    [anon_sym_uimage2DArray] = ACTIONS(1),
    [anon_sym_uimage2DMS] = ACTIONS(1),
    [anon_sym_uimage2DMSArray] = ACTIONS(1),
    [anon_sym_uimage2DRect] = ACTIONS(1),
    [anon_sym_uimage3D] = ACTIONS(1),
    [anon_sym_uimageBuffer] = ACTIONS(1),
    [anon_sym_uimageCube] = ACTIONS(1),
    [anon_sym_uimageCubeArray] = ACTIONS(1),
    [anon_sym_union] = ACTIONS(1),
    [anon_sym_unless] = ACTIONS(1),
    [anon_sym_unorm] = ACTIONS(1),
    [anon_sym_unsafe] = ACTIONS(1),
    [anon_sym_unsigned] = ACTIONS(1),
    [anon_sym_unsized] = ACTIONS(1),
    [anon_sym_usampler1D] = ACTIONS(1),
    [anon_sym_usampler1DArray] = ACTIONS(1),
    [anon_sym_usampler2D] = ACTIONS(1),
    [anon_sym_usampler2DArray] = ACTIONS(1),
    [anon_sym_usampler2DMS] = ACTIONS(1),
    [anon_sym_usampler2DMSArray] = ACTIONS(1),
    [anon_sym_usampler2DRect] = ACTIONS(1),
    [anon_sym_usampler3D] = ACTIONS(1),
    [anon_sym_usamplerBuffer] = ACTIONS(1),
    [anon_sym_usamplerCube] = ACTIONS(1),
    [anon_sym_usamplerCubeArray] = ACTIONS(1),
    [anon_sym_use] = ACTIONS(1),
    [anon_sym_using] = ACTIONS(1),
    [anon_sym_usubpassInput] = ACTIONS(1),
    [anon_sym_usubpassInputMS] = ACTIONS(1),
    [anon_sym_utexture1D] = ACTIONS(1),
    [anon_sym_utexture1DArray] = ACTIONS(1),
    [anon_sym_utexture2D] = ACTIONS(1),
    [anon_sym_utexture2DArray] = ACTIONS(1),
    [anon_sym_utexture2DMS] = ACTIONS(1),
    [anon_sym_utexture2DMSArray] = ACTIONS(1),
    [anon_sym_utexture2DRect] = ACTIONS(1),
    [anon_sym_utexture3D] = ACTIONS(1),
    [anon_sym_utextureBuffer] = ACTIONS(1),
    [anon_sym_utextureCube] = ACTIONS(1),
    [anon_sym_utextureCubeArray] = ACTIONS(1),
    [anon_sym_varying] = ACTIONS(1),
    [anon_sym_vec] = ACTIONS(1),
    [anon_sym_vector] = ACTIONS(1),
    [anon_sym_vertexfragment] = ACTIONS(1),
    [anon_sym_virtual] = ACTIONS(1),
    [anon_sym_void] = ACTIONS(1),
    [anon_sym_volatile] = ACTIONS(1),
    [anon_sym_wchar_t] = ACTIONS(1),
    [anon_sym_wgsl] = ACTIONS(1),
    [anon_sym_where] = ACTIONS(1),
    [anon_sym_with] = ACTIONS(1),
    [anon_sym_writeonly] = ACTIONS(1),
    [anon_sym_yield] = ACTIONS(1),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [1] = {
    [sym_translation_unit] = STATE(384),
    [sym_global_directive] = STATE(173),
    [sym_attribute] = STATE(75),
    [sym_struct_decl] = STATE(176),
    [sym_type_alias_decl] = STATE(377),
    [sym_variable_decl] = STATE(260),
    [sym_global_variable_decl] = STATE(377),
    [sym_global_constant_decl] = STATE(377),
    [sym_static_assert_statement] = STATE(377),
    [sym_function_decl] = STATE(176),
    [sym_function_header] = STATE(263),
    [sym_enable_directive] = STATE(175),
    [sym__comment] = STATE(1),
    [aux_sym_translation_unit_repeat1] = STATE(85),
    [aux_sym_translation_unit_repeat2] = STATE(124),
    [aux_sym_struct_member_repeat1] = STATE(190),
    [ts_builtin_sym_end] = ACTIONS(7),
    [sym_override] = ACTIONS(9),
    [sym_staticAssert] = ACTIONS(11),
    [sym_struct] = ACTIONS(13),
    [sym_const] = ACTIONS(15),
    [sym_enable] = ACTIONS(17),
    [sym_fn] = ACTIONS(19),
    [sym_type] = ACTIONS(21),
    [sym_var] = ACTIONS(23),
    [sym_attr] = ACTIONS(25),
    [sym_semicolon] = ACTIONS(27),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [2] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(242),
    [sym__comment] = STATE(2),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_brace_left] = ACTIONS(69),
    [sym_colon] = ACTIONS(69),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [3] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(242),
    [sym__comment] = STATE(3),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_brace_left] = ACTIONS(73),
    [sym_colon] = ACTIONS(73),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [4] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(300),
    [sym__comment] = STATE(4),
    [aux_sym_argument_expression_list_repeat1] = STATE(8),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_paren_right] = ACTIONS(75),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [5] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(286),
    [sym__comment] = STATE(5),
    [aux_sym_argument_expression_list_repeat1] = STATE(11),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_paren_right] = ACTIONS(77),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [6] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(339),
    [sym__comment] = STATE(6),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_semicolon] = ACTIONS(79),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [7] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(356),
    [sym__comment] = STATE(7),
    [aux_sym_argument_expression_list_repeat1] = STATE(7),
    [sym_ident] = ACTIONS(81),
    [aux_sym_int_literal_token1] = ACTIONS(84),
    [aux_sym_int_literal_token2] = ACTIONS(84),
    [aux_sym_int_literal_token3] = ACTIONS(84),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(87),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(90),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(87),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(87),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(87),
    [aux_sym_hex_float_literal_token1] = ACTIONS(93),
    [aux_sym_hex_float_literal_token2] = ACTIONS(96),
    [aux_sym_hex_float_literal_token3] = ACTIONS(93),
    [sym_array] = ACTIONS(99),
    [sym_atomic] = ACTIONS(102),
    [sym_bool] = ACTIONS(105),
    [sym_float32] = ACTIONS(105),
    [sym_float16] = ACTIONS(105),
    [sym_int32] = ACTIONS(105),
    [sym_mat2x2] = ACTIONS(108),
    [sym_mat2x3] = ACTIONS(108),
    [sym_mat2x4] = ACTIONS(108),
    [sym_mat3x2] = ACTIONS(108),
    [sym_mat3x3] = ACTIONS(108),
    [sym_mat3x4] = ACTIONS(108),
    [sym_mat4x2] = ACTIONS(108),
    [sym_mat4x3] = ACTIONS(108),
    [sym_mat4x4] = ACTIONS(108),
    [sym_pointer] = ACTIONS(111),
    [sym_sampler] = ACTIONS(114),
    [sym_sampler_comparison] = ACTIONS(114),
    [sym_texture_1d] = ACTIONS(117),
    [sym_texture_2d] = ACTIONS(117),
    [sym_texture_2d_array] = ACTIONS(117),
    [sym_texture_3d] = ACTIONS(117),
    [sym_texture_cube] = ACTIONS(117),
    [sym_texture_cube_array] = ACTIONS(117),
    [sym_texture_multisampled_2d] = ACTIONS(120),
    [sym_texture_storage_1d] = ACTIONS(123),
    [sym_texture_storage_2d] = ACTIONS(123),
    [sym_texture_storage_2d_array] = ACTIONS(123),
    [sym_texture_storage_3d] = ACTIONS(123),
    [sym_texture_depth_2d] = ACTIONS(126),
    [sym_texture_depth_2d_array] = ACTIONS(126),
    [sym_texture_depth_cube] = ACTIONS(126),
    [sym_texture_depth_cube_array] = ACTIONS(126),
    [sym_texture_depth_multisampled_2d] = ACTIONS(126),
    [sym_uint32] = ACTIONS(105),
    [sym_vec2] = ACTIONS(129),
    [sym_vec3] = ACTIONS(129),
    [sym_vec4] = ACTIONS(129),
    [sym_bitcast] = ACTIONS(132),
    [sym_false] = ACTIONS(135),
    [sym_true] = ACTIONS(135),
    [sym_and] = ACTIONS(138),
    [sym_bang] = ACTIONS(138),
    [sym_minus] = ACTIONS(138),
    [sym_paren_left] = ACTIONS(141),
    [sym_star] = ACTIONS(138),
    [sym_tilde] = ACTIONS(138),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [8] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(304),
    [sym__comment] = STATE(8),
    [aux_sym_argument_expression_list_repeat1] = STATE(7),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [9] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(327),
    [sym__comment] = STATE(9),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_semicolon] = ACTIONS(144),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [10] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(240),
    [sym_case_selectors] = STATE(244),
    [sym__comment] = STATE(10),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [11] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(283),
    [sym__comment] = STATE(11),
    [aux_sym_argument_expression_list_repeat1] = STATE(7),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [12] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(382),
    [sym__comment] = STATE(12),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_semicolon] = ACTIONS(146),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [13] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(273),
    [sym__comment] = STATE(13),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [14] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(264),
    [sym__comment] = STATE(14),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [15] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(410),
    [sym__comment] = STATE(15),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [16] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(381),
    [sym__comment] = STATE(16),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [17] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(368),
    [sym__comment] = STATE(17),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [18] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(340),
    [sym__comment] = STATE(18),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [19] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(402),
    [sym__comment] = STATE(19),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [20] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(398),
    [sym__comment] = STATE(20),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [21] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(270),
    [sym__comment] = STATE(21),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [22] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(242),
    [sym__comment] = STATE(22),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [23] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(417),
    [sym__comment] = STATE(23),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [24] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(401),
    [sym__comment] = STATE(24),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [25] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(390),
    [sym__comment] = STATE(25),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [26] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(361),
    [sym__comment] = STATE(26),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [27] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(362),
    [sym__comment] = STATE(27),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [28] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(374),
    [sym__comment] = STATE(28),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [29] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(375),
    [sym__comment] = STATE(29),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [30] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(119),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(189),
    [sym_short_circuit_and_expression] = STATE(369),
    [sym_short_circuit_or_expression] = STATE(370),
    [sym_binary_or_expression] = STATE(371),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(373),
    [sym_bitwise_expression] = STATE(220),
    [sym_expression] = STATE(347),
    [sym__comment] = STATE(30),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [31] = {
    [sym_bool_literal] = STATE(142),
    [sym_int_literal] = STATE(142),
    [sym_float_literal] = STATE(142),
    [sym_decimal_float_literal] = STATE(147),
    [sym_hex_float_literal] = STATE(147),
    [sym_const_literal] = STATE(146),
    [sym_array_type_decl] = STATE(199),
    [sym_element_count_expression] = STATE(318),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(171),
    [sym_callable] = STATE(295),
    [sym_paren_expression] = STATE(146),
    [sym_unary_expression] = STATE(186),
    [sym_singular_expression] = STATE(154),
    [sym_multiplicative_expression] = STATE(214),
    [sym_additive_expression] = STATE(241),
    [sym_binary_or_expression] = STATE(396),
    [sym_binary_and_expression] = STATE(372),
    [sym_binary_xor_expression] = STATE(397),
    [sym_bitwise_expression] = STATE(360),
    [sym__comment] = STATE(31),
    [sym_ident] = ACTIONS(148),
    [aux_sym_int_literal_token1] = ACTIONS(150),
    [aux_sym_int_literal_token2] = ACTIONS(150),
    [aux_sym_int_literal_token3] = ACTIONS(150),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(152),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(154),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(152),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(152),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(152),
    [aux_sym_hex_float_literal_token1] = ACTIONS(156),
    [aux_sym_hex_float_literal_token2] = ACTIONS(158),
    [aux_sym_hex_float_literal_token3] = ACTIONS(156),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(160),
    [sym_false] = ACTIONS(162),
    [sym_true] = ACTIONS(162),
    [sym_and] = ACTIONS(164),
    [sym_bang] = ACTIONS(164),
    [sym_minus] = ACTIONS(164),
    [sym_paren_left] = ACTIONS(166),
    [sym_star] = ACTIONS(164),
    [sym_tilde] = ACTIONS(164),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [32] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(127),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(208),
    [sym__comment] = STATE(32),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [33] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(127),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(150),
    [sym_relational_expression] = STATE(209),
    [sym__comment] = STATE(33),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [34] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(127),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(130),
    [sym_additive_expression] = STATE(177),
    [sym__comment] = STATE(34),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [35] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(127),
    [sym_singular_expression] = STATE(116),
    [sym_multiplicative_expression] = STATE(129),
    [sym__comment] = STATE(35),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [36] = {
    [sym_bool_literal] = STATE(142),
    [sym_int_literal] = STATE(142),
    [sym_float_literal] = STATE(142),
    [sym_decimal_float_literal] = STATE(147),
    [sym_hex_float_literal] = STATE(147),
    [sym_const_literal] = STATE(146),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(171),
    [sym_callable] = STATE(295),
    [sym_paren_expression] = STATE(146),
    [sym_unary_expression] = STATE(216),
    [sym_singular_expression] = STATE(154),
    [sym_multiplicative_expression] = STATE(215),
    [sym__comment] = STATE(36),
    [sym_ident] = ACTIONS(148),
    [aux_sym_int_literal_token1] = ACTIONS(150),
    [aux_sym_int_literal_token2] = ACTIONS(150),
    [aux_sym_int_literal_token3] = ACTIONS(150),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(152),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(154),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(152),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(152),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(152),
    [aux_sym_hex_float_literal_token1] = ACTIONS(156),
    [aux_sym_hex_float_literal_token2] = ACTIONS(158),
    [aux_sym_hex_float_literal_token3] = ACTIONS(156),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(160),
    [sym_false] = ACTIONS(162),
    [sym_true] = ACTIONS(162),
    [sym_and] = ACTIONS(164),
    [sym_bang] = ACTIONS(164),
    [sym_minus] = ACTIONS(164),
    [sym_paren_left] = ACTIONS(166),
    [sym_star] = ACTIONS(164),
    [sym_tilde] = ACTIONS(164),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [37] = {
    [sym_bool_literal] = STATE(142),
    [sym_int_literal] = STATE(142),
    [sym_float_literal] = STATE(142),
    [sym_decimal_float_literal] = STATE(147),
    [sym_hex_float_literal] = STATE(147),
    [sym_const_literal] = STATE(146),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(171),
    [sym_callable] = STATE(295),
    [sym_paren_expression] = STATE(146),
    [sym_unary_expression] = STATE(152),
    [sym_singular_expression] = STATE(154),
    [sym__comment] = STATE(37),
    [sym_ident] = ACTIONS(148),
    [aux_sym_int_literal_token1] = ACTIONS(150),
    [aux_sym_int_literal_token2] = ACTIONS(150),
    [aux_sym_int_literal_token3] = ACTIONS(150),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(152),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(154),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(152),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(152),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(152),
    [aux_sym_hex_float_literal_token1] = ACTIONS(156),
    [aux_sym_hex_float_literal_token2] = ACTIONS(158),
    [aux_sym_hex_float_literal_token3] = ACTIONS(156),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(160),
    [sym_false] = ACTIONS(162),
    [sym_true] = ACTIONS(162),
    [sym_and] = ACTIONS(164),
    [sym_bang] = ACTIONS(164),
    [sym_minus] = ACTIONS(164),
    [sym_paren_left] = ACTIONS(166),
    [sym_star] = ACTIONS(164),
    [sym_tilde] = ACTIONS(164),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [38] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(197),
    [sym_singular_expression] = STATE(116),
    [sym__comment] = STATE(38),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [39] = {
    [sym_bool_literal] = STATE(142),
    [sym_int_literal] = STATE(142),
    [sym_float_literal] = STATE(142),
    [sym_decimal_float_literal] = STATE(147),
    [sym_hex_float_literal] = STATE(147),
    [sym_const_literal] = STATE(146),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(163),
    [sym_callable] = STATE(295),
    [sym_paren_expression] = STATE(146),
    [sym_unary_expression] = STATE(198),
    [sym_singular_expression] = STATE(154),
    [sym__comment] = STATE(39),
    [sym_ident] = ACTIONS(148),
    [aux_sym_int_literal_token1] = ACTIONS(150),
    [aux_sym_int_literal_token2] = ACTIONS(150),
    [aux_sym_int_literal_token3] = ACTIONS(150),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(152),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(154),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(152),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(152),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(152),
    [aux_sym_hex_float_literal_token1] = ACTIONS(156),
    [aux_sym_hex_float_literal_token2] = ACTIONS(158),
    [aux_sym_hex_float_literal_token3] = ACTIONS(156),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(160),
    [sym_false] = ACTIONS(162),
    [sym_true] = ACTIONS(162),
    [sym_and] = ACTIONS(168),
    [sym_bang] = ACTIONS(168),
    [sym_minus] = ACTIONS(168),
    [sym_paren_left] = ACTIONS(166),
    [sym_star] = ACTIONS(168),
    [sym_tilde] = ACTIONS(168),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [40] = {
    [sym_bool_literal] = STATE(142),
    [sym_int_literal] = STATE(142),
    [sym_float_literal] = STATE(142),
    [sym_decimal_float_literal] = STATE(147),
    [sym_hex_float_literal] = STATE(147),
    [sym_const_literal] = STATE(146),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(163),
    [sym_callable] = STATE(295),
    [sym_paren_expression] = STATE(146),
    [sym_unary_expression] = STATE(152),
    [sym_singular_expression] = STATE(154),
    [sym__comment] = STATE(40),
    [sym_ident] = ACTIONS(148),
    [aux_sym_int_literal_token1] = ACTIONS(150),
    [aux_sym_int_literal_token2] = ACTIONS(150),
    [aux_sym_int_literal_token3] = ACTIONS(150),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(152),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(154),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(152),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(152),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(152),
    [aux_sym_hex_float_literal_token1] = ACTIONS(156),
    [aux_sym_hex_float_literal_token2] = ACTIONS(158),
    [aux_sym_hex_float_literal_token3] = ACTIONS(156),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(160),
    [sym_false] = ACTIONS(162),
    [sym_true] = ACTIONS(162),
    [sym_and] = ACTIONS(168),
    [sym_bang] = ACTIONS(168),
    [sym_minus] = ACTIONS(168),
    [sym_paren_left] = ACTIONS(166),
    [sym_star] = ACTIONS(168),
    [sym_tilde] = ACTIONS(168),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [41] = {
    [sym_bool_literal] = STATE(142),
    [sym_int_literal] = STATE(142),
    [sym_float_literal] = STATE(142),
    [sym_decimal_float_literal] = STATE(147),
    [sym_hex_float_literal] = STATE(147),
    [sym_const_literal] = STATE(146),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(171),
    [sym_callable] = STATE(295),
    [sym_paren_expression] = STATE(146),
    [sym_unary_expression] = STATE(218),
    [sym_singular_expression] = STATE(154),
    [sym__comment] = STATE(41),
    [sym_ident] = ACTIONS(148),
    [aux_sym_int_literal_token1] = ACTIONS(150),
    [aux_sym_int_literal_token2] = ACTIONS(150),
    [aux_sym_int_literal_token3] = ACTIONS(150),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(152),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(154),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(152),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(152),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(152),
    [aux_sym_hex_float_literal_token1] = ACTIONS(156),
    [aux_sym_hex_float_literal_token2] = ACTIONS(158),
    [aux_sym_hex_float_literal_token3] = ACTIONS(156),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(160),
    [sym_false] = ACTIONS(162),
    [sym_true] = ACTIONS(162),
    [sym_and] = ACTIONS(164),
    [sym_bang] = ACTIONS(164),
    [sym_minus] = ACTIONS(164),
    [sym_paren_left] = ACTIONS(166),
    [sym_star] = ACTIONS(164),
    [sym_tilde] = ACTIONS(164),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [42] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(201),
    [sym_singular_expression] = STATE(116),
    [sym__comment] = STATE(42),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [43] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(118),
    [sym_singular_expression] = STATE(116),
    [sym__comment] = STATE(43),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [44] = {
    [sym_bool_literal] = STATE(142),
    [sym_int_literal] = STATE(142),
    [sym_float_literal] = STATE(142),
    [sym_decimal_float_literal] = STATE(147),
    [sym_hex_float_literal] = STATE(147),
    [sym_const_literal] = STATE(146),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(163),
    [sym_callable] = STATE(295),
    [sym_paren_expression] = STATE(146),
    [sym_unary_expression] = STATE(201),
    [sym_singular_expression] = STATE(154),
    [sym__comment] = STATE(44),
    [sym_ident] = ACTIONS(148),
    [aux_sym_int_literal_token1] = ACTIONS(150),
    [aux_sym_int_literal_token2] = ACTIONS(150),
    [aux_sym_int_literal_token3] = ACTIONS(150),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(152),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(154),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(152),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(152),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(152),
    [aux_sym_hex_float_literal_token1] = ACTIONS(156),
    [aux_sym_hex_float_literal_token2] = ACTIONS(158),
    [aux_sym_hex_float_literal_token3] = ACTIONS(156),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(160),
    [sym_false] = ACTIONS(162),
    [sym_true] = ACTIONS(162),
    [sym_and] = ACTIONS(168),
    [sym_bang] = ACTIONS(168),
    [sym_minus] = ACTIONS(168),
    [sym_paren_left] = ACTIONS(166),
    [sym_star] = ACTIONS(168),
    [sym_tilde] = ACTIONS(168),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [45] = {
    [sym_bool_literal] = STATE(142),
    [sym_int_literal] = STATE(142),
    [sym_float_literal] = STATE(142),
    [sym_decimal_float_literal] = STATE(147),
    [sym_hex_float_literal] = STATE(147),
    [sym_const_literal] = STATE(146),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(163),
    [sym_callable] = STATE(295),
    [sym_paren_expression] = STATE(146),
    [sym_unary_expression] = STATE(197),
    [sym_singular_expression] = STATE(154),
    [sym__comment] = STATE(45),
    [sym_ident] = ACTIONS(148),
    [aux_sym_int_literal_token1] = ACTIONS(150),
    [aux_sym_int_literal_token2] = ACTIONS(150),
    [aux_sym_int_literal_token3] = ACTIONS(150),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(152),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(154),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(152),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(152),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(152),
    [aux_sym_hex_float_literal_token1] = ACTIONS(156),
    [aux_sym_hex_float_literal_token2] = ACTIONS(158),
    [aux_sym_hex_float_literal_token3] = ACTIONS(156),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(160),
    [sym_false] = ACTIONS(162),
    [sym_true] = ACTIONS(162),
    [sym_and] = ACTIONS(168),
    [sym_bang] = ACTIONS(168),
    [sym_minus] = ACTIONS(168),
    [sym_paren_left] = ACTIONS(166),
    [sym_star] = ACTIONS(168),
    [sym_tilde] = ACTIONS(168),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
  [46] = {
    [sym_bool_literal] = STATE(94),
    [sym_int_literal] = STATE(94),
    [sym_float_literal] = STATE(94),
    [sym_decimal_float_literal] = STATE(87),
    [sym_hex_float_literal] = STATE(87),
    [sym_const_literal] = STATE(92),
    [sym_array_type_decl] = STATE(199),
    [sym_texture_sampler_types] = STATE(199),
    [sym_sampler_type] = STATE(203),
    [sym_sampled_texture_type] = STATE(350),
    [sym_multisampled_texture_type] = STATE(350),
    [sym_storage_texture_type] = STATE(357),
    [sym_depth_texture_type] = STATE(203),
    [sym_type_decl_without_ident] = STATE(358),
    [sym_vec_prefix] = STATE(272),
    [sym_mat_prefix] = STATE(272),
    [sym_primary_expression] = STATE(82),
    [sym_callable] = STATE(281),
    [sym_paren_expression] = STATE(92),
    [sym_unary_expression] = STATE(128),
    [sym_singular_expression] = STATE(116),
    [sym__comment] = STATE(46),
    [sym_ident] = ACTIONS(29),
    [aux_sym_int_literal_token1] = ACTIONS(31),
    [aux_sym_int_literal_token2] = ACTIONS(31),
    [aux_sym_int_literal_token3] = ACTIONS(31),
    [aux_sym_decimal_float_literal_token1] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token2] = ACTIONS(35),
    [aux_sym_decimal_float_literal_token3] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token4] = ACTIONS(33),
    [aux_sym_decimal_float_literal_token5] = ACTIONS(33),
    [aux_sym_hex_float_literal_token1] = ACTIONS(37),
    [aux_sym_hex_float_literal_token2] = ACTIONS(39),
    [aux_sym_hex_float_literal_token3] = ACTIONS(37),
    [sym_array] = ACTIONS(41),
    [sym_atomic] = ACTIONS(43),
    [sym_bool] = ACTIONS(45),
    [sym_float32] = ACTIONS(45),
    [sym_float16] = ACTIONS(45),
    [sym_int32] = ACTIONS(45),
    [sym_mat2x2] = ACTIONS(47),
    [sym_mat2x3] = ACTIONS(47),
    [sym_mat2x4] = ACTIONS(47),
    [sym_mat3x2] = ACTIONS(47),
    [sym_mat3x3] = ACTIONS(47),
    [sym_mat3x4] = ACTIONS(47),
    [sym_mat4x2] = ACTIONS(47),
    [sym_mat4x3] = ACTIONS(47),
    [sym_mat4x4] = ACTIONS(47),
    [sym_pointer] = ACTIONS(49),
    [sym_sampler] = ACTIONS(51),
    [sym_sampler_comparison] = ACTIONS(51),
    [sym_texture_1d] = ACTIONS(53),
    [sym_texture_2d] = ACTIONS(53),
    [sym_texture_2d_array] = ACTIONS(53),
    [sym_texture_3d] = ACTIONS(53),
    [sym_texture_cube] = ACTIONS(53),
    [sym_texture_cube_array] = ACTIONS(53),
    [sym_texture_multisampled_2d] = ACTIONS(55),
    [sym_texture_storage_1d] = ACTIONS(57),
    [sym_texture_storage_2d] = ACTIONS(57),
    [sym_texture_storage_2d_array] = ACTIONS(57),
    [sym_texture_storage_3d] = ACTIONS(57),
    [sym_texture_depth_2d] = ACTIONS(59),
    [sym_texture_depth_2d_array] = ACTIONS(59),
    [sym_texture_depth_cube] = ACTIONS(59),
    [sym_texture_depth_cube_array] = ACTIONS(59),
    [sym_texture_depth_multisampled_2d] = ACTIONS(59),
    [sym_uint32] = ACTIONS(45),
    [sym_vec2] = ACTIONS(61),
    [sym_vec3] = ACTIONS(61),
    [sym_vec4] = ACTIONS(61),
    [sym_bitcast] = ACTIONS(63),
    [sym_false] = ACTIONS(65),
    [sym_true] = ACTIONS(65),
    [sym_and] = ACTIONS(67),
    [sym_bang] = ACTIONS(67),
    [sym_minus] = ACTIONS(67),
    [sym_paren_left] = ACTIONS(71),
    [sym_star] = ACTIONS(67),
    [sym_tilde] = ACTIONS(67),
    [anon_sym_SLASH_SLASH] = ACTIONS(3),
    [sym__blankspace] = ACTIONS(5),
    [sym__block_comment] = ACTIONS(5),
  },
};

static const uint16_t ts_small_parse_table[] = {
  [0] = 6,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(174), 1,
      sym_paren_right,
    STATE(47), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(172), 12,
      aux_sym_decimal_float_literal_token1,
      aux_sym_decimal_float_literal_token3,
      aux_sym_decimal_float_literal_token4,
      aux_sym_decimal_float_literal_token5,
      aux_sym_hex_float_literal_token1,
      aux_sym_hex_float_literal_token3,
      sym_and,
      sym_bang,
      sym_minus,
      sym_paren_left,
      sym_star,
      sym_tilde,
    ACTIONS(170), 47,
      aux_sym_int_literal_token1,
      aux_sym_int_literal_token2,
      aux_sym_int_literal_token3,
      aux_sym_decimal_float_literal_token2,
      aux_sym_hex_float_literal_token2,
      sym_array,
      sym_atomic,
      sym_bool,
      sym_float32,
      sym_float16,
      sym_int32,
      sym_mat2x2,
      sym_mat2x3,
      sym_mat2x4,
      sym_mat3x2,
      sym_mat3x3,
      sym_mat3x4,
      sym_mat4x2,
      sym_mat4x3,
      sym_mat4x4,
      sym_pointer,
      sym_sampler,
      sym_sampler_comparison,
      sym_texture_1d,
      sym_texture_2d,
      sym_texture_2d_array,
      sym_texture_3d,
      sym_texture_cube,
      sym_texture_cube_array,
      sym_texture_multisampled_2d,
      sym_texture_storage_1d,
      sym_texture_storage_2d,
      sym_texture_storage_2d_array,
      sym_texture_storage_3d,
      sym_texture_depth_2d,
      sym_texture_depth_2d_array,
      sym_texture_depth_cube,
      sym_texture_depth_cube_array,
      sym_texture_depth_multisampled_2d,
      sym_uint32,
      sym_vec2,
      sym_vec3,
      sym_vec4,
      sym_bitcast,
      sym_false,
      sym_true,
      sym_ident,
  [77] = 6,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(176), 1,
      sym_paren_right,
    STATE(48), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(172), 12,
      aux_sym_decimal_float_literal_token1,
      aux_sym_decimal_float_literal_token3,
      aux_sym_decimal_float_literal_token4,
      aux_sym_decimal_float_literal_token5,
      aux_sym_hex_float_literal_token1,
      aux_sym_hex_float_literal_token3,
      sym_and,
      sym_bang,
      sym_minus,
      sym_paren_left,
      sym_star,
      sym_tilde,
    ACTIONS(170), 47,
      aux_sym_int_literal_token1,
      aux_sym_int_literal_token2,
      aux_sym_int_literal_token3,
      aux_sym_decimal_float_literal_token2,
      aux_sym_hex_float_literal_token2,
      sym_array,
      sym_atomic,
      sym_bool,
      sym_float32,
      sym_float16,
      sym_int32,
      sym_mat2x2,
      sym_mat2x3,
      sym_mat2x4,
      sym_mat3x2,
      sym_mat3x3,
      sym_mat3x4,
      sym_mat4x2,
      sym_mat4x3,
      sym_mat4x4,
      sym_pointer,
      sym_sampler,
      sym_sampler_comparison,
      sym_texture_1d,
      sym_texture_2d,
      sym_texture_2d_array,
      sym_texture_3d,
      sym_texture_cube,
      sym_texture_cube_array,
      sym_texture_multisampled_2d,
      sym_texture_storage_1d,
      sym_texture_storage_2d,
      sym_texture_storage_2d_array,
      sym_texture_storage_3d,
      sym_texture_depth_2d,
      sym_texture_depth_2d_array,
      sym_texture_depth_cube,
      sym_texture_depth_cube_array,
      sym_texture_depth_multisampled_2d,
      sym_uint32,
      sym_vec2,
      sym_vec3,
      sym_vec4,
      sym_bitcast,
      sym_false,
      sym_true,
      sym_ident,
  [154] = 6,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(178), 1,
      sym_paren_right,
    STATE(49), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(172), 12,
      aux_sym_decimal_float_literal_token1,
      aux_sym_decimal_float_literal_token3,
      aux_sym_decimal_float_literal_token4,
      aux_sym_decimal_float_literal_token5,
      aux_sym_hex_float_literal_token1,
      aux_sym_hex_float_literal_token3,
      sym_and,
      sym_bang,
      sym_minus,
      sym_paren_left,
      sym_star,
      sym_tilde,
    ACTIONS(170), 47,
      aux_sym_int_literal_token1,
      aux_sym_int_literal_token2,
      aux_sym_int_literal_token3,
      aux_sym_decimal_float_literal_token2,
      aux_sym_hex_float_literal_token2,
      sym_array,
      sym_atomic,
      sym_bool,
      sym_float32,
      sym_float16,
      sym_int32,
      sym_mat2x2,
      sym_mat2x3,
      sym_mat2x4,
      sym_mat3x2,
      sym_mat3x3,
      sym_mat3x4,
      sym_mat4x2,
      sym_mat4x3,
      sym_mat4x4,
      sym_pointer,
      sym_sampler,
      sym_sampler_comparison,
      sym_texture_1d,
      sym_texture_2d,
      sym_texture_2d_array,
      sym_texture_3d,
      sym_texture_cube,
      sym_texture_cube_array,
      sym_texture_multisampled_2d,
      sym_texture_storage_1d,
      sym_texture_storage_2d,
      sym_texture_storage_2d_array,
      sym_texture_storage_3d,
      sym_texture_depth_2d,
      sym_texture_depth_2d_array,
      sym_texture_depth_cube,
      sym_texture_depth_cube_array,
      sym_texture_depth_multisampled_2d,
      sym_uint32,
      sym_vec2,
      sym_vec3,
      sym_vec4,
      sym_bitcast,
      sym_false,
      sym_true,
      sym_ident,
  [231] = 6,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(180), 1,
      sym_paren_right,
    STATE(50), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(172), 12,
      aux_sym_decimal_float_literal_token1,
      aux_sym_decimal_float_literal_token3,
      aux_sym_decimal_float_literal_token4,
      aux_sym_decimal_float_literal_token5,
      aux_sym_hex_float_literal_token1,
      aux_sym_hex_float_literal_token3,
      sym_and,
      sym_bang,
      sym_minus,
      sym_paren_left,
      sym_star,
      sym_tilde,
    ACTIONS(170), 47,
      aux_sym_int_literal_token1,
      aux_sym_int_literal_token2,
      aux_sym_int_literal_token3,
      aux_sym_decimal_float_literal_token2,
      aux_sym_hex_float_literal_token2,
      sym_array,
      sym_atomic,
      sym_bool,
      sym_float32,
      sym_float16,
      sym_int32,
      sym_mat2x2,
      sym_mat2x3,
      sym_mat2x4,
      sym_mat3x2,
      sym_mat3x3,
      sym_mat3x4,
      sym_mat4x2,
      sym_mat4x3,
      sym_mat4x4,
      sym_pointer,
      sym_sampler,
      sym_sampler_comparison,
      sym_texture_1d,
      sym_texture_2d,
      sym_texture_2d_array,
      sym_texture_3d,
      sym_texture_cube,
      sym_texture_cube_array,
      sym_texture_multisampled_2d,
      sym_texture_storage_1d,
      sym_texture_storage_2d,
      sym_texture_storage_2d_array,
      sym_texture_storage_3d,
      sym_texture_depth_2d,
      sym_texture_depth_2d_array,
      sym_texture_depth_cube,
      sym_texture_depth_cube_array,
      sym_texture_depth_multisampled_2d,
      sym_uint32,
      sym_vec2,
      sym_vec3,
      sym_vec4,
      sym_bitcast,
      sym_false,
      sym_true,
      sym_ident,
  [308] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(51), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(184), 12,
      aux_sym_decimal_float_literal_token1,
      aux_sym_decimal_float_literal_token3,
      aux_sym_decimal_float_literal_token4,
      aux_sym_decimal_float_literal_token5,
      aux_sym_hex_float_literal_token1,
      aux_sym_hex_float_literal_token3,
      sym_and,
      sym_bang,
      sym_minus,
      sym_paren_left,
      sym_star,
      sym_tilde,
    ACTIONS(182), 47,
      aux_sym_int_literal_token1,
      aux_sym_int_literal_token2,
      aux_sym_int_literal_token3,
      aux_sym_decimal_float_literal_token2,
      aux_sym_hex_float_literal_token2,
      sym_array,
      sym_atomic,
      sym_bool,
      sym_float32,
      sym_float16,
      sym_int32,
      sym_mat2x2,
      sym_mat2x3,
      sym_mat2x4,
      sym_mat3x2,
      sym_mat3x3,
      sym_mat3x4,
      sym_mat4x2,
      sym_mat4x3,
      sym_mat4x4,
      sym_pointer,
      sym_sampler,
      sym_sampler_comparison,
      sym_texture_1d,
      sym_texture_2d,
      sym_texture_2d_array,
      sym_texture_3d,
      sym_texture_cube,
      sym_texture_cube_array,
      sym_texture_multisampled_2d,
      sym_texture_storage_1d,
      sym_texture_storage_2d,
      sym_texture_storage_2d_array,
      sym_texture_storage_3d,
      sym_texture_depth_2d,
      sym_texture_depth_2d_array,
      sym_texture_depth_cube,
      sym_texture_depth_cube_array,
      sym_texture_depth_multisampled_2d,
      sym_uint32,
      sym_vec2,
      sym_vec3,
      sym_vec4,
      sym_bitcast,
      sym_false,
      sym_true,
      sym_ident,
  [382] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(52), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(172), 12,
      aux_sym_decimal_float_literal_token1,
      aux_sym_decimal_float_literal_token3,
      aux_sym_decimal_float_literal_token4,
      aux_sym_decimal_float_literal_token5,
      aux_sym_hex_float_literal_token1,
      aux_sym_hex_float_literal_token3,
      sym_and,
      sym_bang,
      sym_minus,
      sym_paren_left,
      sym_star,
      sym_tilde,
    ACTIONS(170), 47,
      aux_sym_int_literal_token1,
      aux_sym_int_literal_token2,
      aux_sym_int_literal_token3,
      aux_sym_decimal_float_literal_token2,
      aux_sym_hex_float_literal_token2,
      sym_array,
      sym_atomic,
      sym_bool,
      sym_float32,
      sym_float16,
      sym_int32,
      sym_mat2x2,
      sym_mat2x3,
      sym_mat2x4,
      sym_mat3x2,
      sym_mat3x3,
      sym_mat3x4,
      sym_mat4x2,
      sym_mat4x3,
      sym_mat4x4,
      sym_pointer,
      sym_sampler,
      sym_sampler_comparison,
      sym_texture_1d,
      sym_texture_2d,
      sym_texture_2d_array,
      sym_texture_3d,
      sym_texture_cube,
      sym_texture_cube_array,
      sym_texture_multisampled_2d,
      sym_texture_storage_1d,
      sym_texture_storage_2d,
      sym_texture_storage_2d_array,
      sym_texture_storage_3d,
      sym_texture_depth_2d,
      sym_texture_depth_2d_array,
      sym_texture_depth_cube,
      sym_texture_depth_cube_array,
      sym_texture_depth_multisampled_2d,
      sym_uint32,
      sym_vec2,
      sym_vec3,
      sym_vec4,
      sym_bitcast,
      sym_false,
      sym_true,
      sym_ident,
  [456] = 25,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(25), 1,
      sym_attr,
    ACTIONS(41), 1,
      sym_array,
    ACTIONS(43), 1,
      sym_atomic,
    ACTIONS(49), 1,
      sym_pointer,
    ACTIONS(55), 1,
      sym_texture_multisampled_2d,
    ACTIONS(186), 1,
      sym_ident,
    STATE(53), 1,
      sym__comment,
    STATE(55), 1,
      aux_sym_struct_member_repeat1,
    STATE(75), 1,
      sym_attribute,
    STATE(212), 1,
      sym_type_decl_without_ident,
    STATE(351), 1,
      sym_type_decl,
    STATE(357), 1,
      sym_storage_texture_type,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(51), 2,
      sym_sampler,
      sym_sampler_comparison,
    STATE(199), 2,
      sym_array_type_decl,
      sym_texture_sampler_types,
    STATE(203), 2,
      sym_sampler_type,
      sym_depth_texture_type,
    STATE(325), 2,
      sym_vec_prefix,
      sym_mat_prefix,
    STATE(350), 2,
      sym_sampled_texture_type,
      sym_multisampled_texture_type,
    ACTIONS(61), 3,
      sym_vec2,
      sym_vec3,
      sym_vec4,
    ACTIONS(57), 4,
      sym_texture_storage_1d,
      sym_texture_storage_2d,
      sym_texture_storage_2d_array,
      sym_texture_storage_3d,
    ACTIONS(45), 5,
      sym_bool,
      sym_float32,
      sym_float16,
      sym_int32,
      sym_uint32,
    ACTIONS(59), 5,
      sym_texture_depth_2d,
      sym_texture_depth_2d_array,
      sym_texture_depth_cube,
      sym_texture_depth_cube_array,
      sym_texture_depth_multisampled_2d,
    ACTIONS(53), 6,
      sym_texture_1d,
      sym_texture_2d,
      sym_texture_2d_array,
      sym_texture_3d,
      sym_texture_cube,
      sym_texture_cube_array,
    ACTIONS(47), 9,
      sym_mat2x2,
      sym_mat2x3,
      sym_mat2x4,
      sym_mat3x2,
      sym_mat3x3,
      sym_mat3x4,
      sym_mat4x2,
      sym_mat4x3,
      sym_mat4x4,
  [564] = 25,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(25), 1,
      sym_attr,
    ACTIONS(41), 1,
      sym_array,
    ACTIONS(43), 1,
      sym_atomic,
    ACTIONS(49), 1,
      sym_pointer,
    ACTIONS(55), 1,
      sym_texture_multisampled_2d,
    ACTIONS(186), 1,
      sym_ident,
    STATE(54), 1,
      sym__comment,
    STATE(56), 1,
      aux_sym_struct_member_repeat1,
    STATE(75), 1,
      sym_attribute,
    STATE(212), 1,
      sym_type_decl_without_ident,
    STATE(326), 1,
      sym_type_decl,
    STATE(357), 1,
      sym_storage_texture_type,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(51), 2,
      sym_sampler,
      sym_sampler_comparison,
    STATE(199), 2,
      sym_array_type_decl,
      sym_texture_sampler_types,
    STATE(203), 2,
      sym_sampler_type,
      sym_depth_texture_type,
    STATE(325), 2,
      sym_vec_prefix,
      sym_mat_prefix,
    STATE(350), 2,
      sym_sampled_texture_type,
      sym_multisampled_texture_type,
    ACTIONS(61), 3,
      sym_vec2,
      sym_vec3,
      sym_vec4,
    ACTIONS(57), 4,
      sym_texture_storage_1d,
      sym_texture_storage_2d,
      sym_texture_storage_2d_array,
      sym_texture_storage_3d,
    ACTIONS(45), 5,
      sym_bool,
      sym_float32,
      sym_float16,
      sym_int32,
      sym_uint32,
    ACTIONS(59), 5,
      sym_texture_depth_2d,
      sym_texture_depth_2d_array,
      sym_texture_depth_cube,
      sym_texture_depth_cube_array,
      sym_texture_depth_multisampled_2d,
    ACTIONS(53), 6,
      sym_texture_1d,
      sym_texture_2d,
      sym_texture_2d_array,
      sym_texture_3d,
      sym_texture_cube,
      sym_texture_cube_array,
    ACTIONS(47), 9,
      sym_mat2x2,
      sym_mat2x3,
      sym_mat2x4,
      sym_mat3x2,
      sym_mat3x3,
      sym_mat3x4,
      sym_mat4x2,
      sym_mat4x3,
      sym_mat4x4,
  [672] = 25,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(25), 1,
      sym_attr,
    ACTIONS(41), 1,
      sym_array,
    ACTIONS(43), 1,
      sym_atomic,
    ACTIONS(49), 1,
      sym_pointer,
    ACTIONS(55), 1,
      sym_texture_multisampled_2d,
    ACTIONS(186), 1,
      sym_ident,
    STATE(55), 1,
      sym__comment,
    STATE(65), 1,
      aux_sym_struct_member_repeat1,
    STATE(75), 1,
      sym_attribute,
    STATE(212), 1,
      sym_type_decl_without_ident,
    STATE(326), 1,
      sym_type_decl,
    STATE(357), 1,
      sym_storage_texture_type,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(51), 2,
      sym_sampler,
      sym_sampler_comparison,
    STATE(199), 2,
      sym_array_type_decl,
      sym_texture_sampler_types,
    STATE(203), 2,
      sym_sampler_type,
      sym_depth_texture_type,
    STATE(325), 2,
      sym_vec_prefix,
      sym_mat_prefix,
    STATE(350), 2,
      sym_sampled_texture_type,
      sym_multisampled_texture_type,
    ACTIONS(61), 3,
      sym_vec2,
      sym_vec3,
      sym_vec4,
    ACTIONS(57), 4,
      sym_texture_storage_1d,
      sym_texture_storage_2d,
      sym_texture_storage_2d_array,
      sym_texture_storage_3d,
    ACTIONS(45), 5,
      sym_bool,
      sym_float32,
      sym_float16,
      sym_int32,
      sym_uint32,
    ACTIONS(59), 5,
      sym_texture_depth_2d,
      sym_texture_depth_2d_array,
      sym_texture_depth_cube,
      sym_texture_depth_cube_array,
      sym_texture_depth_multisampled_2d,
    ACTIONS(53), 6,
      sym_texture_1d,
      sym_texture_2d,
      sym_texture_2d_array,
      sym_texture_3d,
      sym_texture_cube,
      sym_texture_cube_array,
    ACTIONS(47), 9,
      sym_mat2x2,
      sym_mat2x3,
      sym_mat2x4,
      sym_mat3x2,
      sym_mat3x3,
      sym_mat3x4,
      sym_mat4x2,
      sym_mat4x3,
      sym_mat4x4,
  [780] = 25,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(25), 1,
      sym_attr,
    ACTIONS(41), 1,
      sym_array,
    ACTIONS(43), 1,
      sym_atomic,
    ACTIONS(49), 1,
      sym_pointer,
    ACTIONS(55), 1,
      sym_texture_multisampled_2d,
    ACTIONS(186), 1,
      sym_ident,
    STATE(56), 1,
      sym__comment,
    STATE(65), 1,
      aux_sym_struct_member_repeat1,
    STATE(75), 1,
      sym_attribute,
    STATE(212), 1,
      sym_type_decl_without_ident,
    STATE(331), 1,
      sym_type_decl,
    STATE(357), 1,
      sym_storage_texture_type,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(51), 2,
      sym_sampler,
      sym_sampler_comparison,
    STATE(199), 2,
      sym_array_type_decl,
      sym_texture_sampler_types,
    STATE(203), 2,
      sym_sampler_type,
      sym_depth_texture_type,
    STATE(325), 2,
      sym_vec_prefix,
      sym_mat_prefix,
    STATE(350), 2,
      sym_sampled_texture_type,
      sym_multisampled_texture_type,
    ACTIONS(61), 3,
      sym_vec2,
      sym_vec3,
      sym_vec4,
    ACTIONS(57), 4,
      sym_texture_storage_1d,
      sym_texture_storage_2d,
      sym_texture_storage_2d_array,
      sym_texture_storage_3d,
    ACTIONS(45), 5,
      sym_bool,
      sym_float32,
      sym_float16,
      sym_int32,
      sym_uint32,
    ACTIONS(59), 5,
      sym_texture_depth_2d,
      sym_texture_depth_2d_array,
      sym_texture_depth_cube,
      sym_texture_depth_cube_array,
      sym_texture_depth_multisampled_2d,
    ACTIONS(53), 6,
      sym_texture_1d,
      sym_texture_2d,
      sym_texture_2d_array,
      sym_texture_3d,
      sym_texture_cube,
      sym_texture_cube_array,
    ACTIONS(47), 9,
      sym_mat2x2,
      sym_mat2x3,
      sym_mat2x4,
      sym_mat3x2,
      sym_mat3x3,
      sym_mat3x4,
      sym_mat4x2,
      sym_mat4x3,
      sym_mat4x4,
  [888] = 22,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(41), 1,
      sym_array,
    ACTIONS(43), 1,
      sym_atomic,
    ACTIONS(49), 1,
      sym_pointer,
    ACTIONS(55), 1,
      sym_texture_multisampled_2d,
    ACTIONS(186), 1,
      sym_ident,
    STATE(57), 1,
      sym__comment,
    STATE(212), 1,
      sym_type_decl_without_ident,
    STATE(357), 1,
      sym_storage_texture_type,
    STATE(394), 1,
      sym_type_decl,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(51), 2,
      sym_sampler,
      sym_sampler_comparison,
    STATE(199), 2,
      sym_array_type_decl,
      sym_texture_sampler_types,
    STATE(203), 2,
      sym_sampler_type,
      sym_depth_texture_type,
    STATE(325), 2,
      sym_vec_prefix,
      sym_mat_prefix,
    STATE(350), 2,
      sym_sampled_texture_type,
      sym_multisampled_texture_type,
    ACTIONS(61), 3,
      sym_vec2,
      sym_vec3,
      sym_vec4,
    ACTIONS(57), 4,
      sym_texture_storage_1d,
      sym_texture_storage_2d,
      sym_texture_storage_2d_array,
      sym_texture_storage_3d,
    ACTIONS(45), 5,
      sym_bool,
      sym_float32,
      sym_float16,
      sym_int32,
      sym_uint32,
    ACTIONS(59), 5,
      sym_texture_depth_2d,
      sym_texture_depth_2d_array,
      sym_texture_depth_cube,
      sym_texture_depth_cube_array,
      sym_texture_depth_multisampled_2d,
    ACTIONS(53), 6,
      sym_texture_1d,
      sym_texture_2d,
      sym_texture_2d_array,
      sym_texture_3d,
      sym_texture_cube,
      sym_texture_cube_array,
    ACTIONS(47), 9,
      sym_mat2x2,
      sym_mat2x3,
      sym_mat2x4,
      sym_mat3x2,
      sym_mat3x3,
      sym_mat3x4,
      sym_mat4x2,
      sym_mat4x3,
      sym_mat4x4,
  [987] = 22,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(41), 1,
      sym_array,
    ACTIONS(43), 1,
      sym_atomic,
    ACTIONS(49), 1,
      sym_pointer,
    ACTIONS(55), 1,
      sym_texture_multisampled_2d,
    ACTIONS(186), 1,
      sym_ident,
    STATE(58), 1,
      sym__comment,
    STATE(212), 1,
      sym_type_decl_without_ident,
    STATE(229), 1,
      sym_type_decl,
    STATE(357), 1,
      sym_storage_texture_type,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(51), 2,
      sym_sampler,
      sym_sampler_comparison,
    STATE(199), 2,
      sym_array_type_decl,
      sym_texture_sampler_types,
    STATE(203), 2,
      sym_sampler_type,
      sym_depth_texture_type,
    STATE(325), 2,
      sym_vec_prefix,
      sym_mat_prefix,
    STATE(350), 2,
      sym_sampled_texture_type,
      sym_multisampled_texture_type,
    ACTIONS(61), 3,
      sym_vec2,
      sym_vec3,
      sym_vec4,
    ACTIONS(57), 4,
      sym_texture_storage_1d,
      sym_texture_storage_2d,
      sym_texture_storage_2d_array,
      sym_texture_storage_3d,
    ACTIONS(45), 5,
      sym_bool,
      sym_float32,
      sym_float16,
      sym_int32,
      sym_uint32,
    ACTIONS(59), 5,
      sym_texture_depth_2d,
      sym_texture_depth_2d_array,
      sym_texture_depth_cube,
      sym_texture_depth_cube_array,
      sym_texture_depth_multisampled_2d,
    ACTIONS(53), 6,
      sym_texture_1d,
      sym_texture_2d,
      sym_texture_2d_array,
      sym_texture_3d,
      sym_texture_cube,
      sym_texture_cube_array,
    ACTIONS(47), 9,
      sym_mat2x2,
      sym_mat2x3,
      sym_mat2x4,
      sym_mat3x2,
      sym_mat3x3,
      sym_mat3x4,
      sym_mat4x2,
      sym_mat4x3,
      sym_mat4x4,
  [1086] = 22,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(41), 1,
      sym_array,
    ACTIONS(43), 1,
      sym_atomic,
    ACTIONS(49), 1,
      sym_pointer,
    ACTIONS(55), 1,
      sym_texture_multisampled_2d,
    ACTIONS(186), 1,
      sym_ident,
    STATE(59), 1,
      sym__comment,
    STATE(212), 1,
      sym_type_decl_without_ident,
    STATE(357), 1,
      sym_storage_texture_type,
    STATE(400), 1,
      sym_type_decl,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(51), 2,
      sym_sampler,
      sym_sampler_comparison,
    STATE(199), 2,
      sym_array_type_decl,
      sym_texture_sampler_types,
    STATE(203), 2,
      sym_sampler_type,
      sym_depth_texture_type,
    STATE(325), 2,
      sym_vec_prefix,
      sym_mat_prefix,
    STATE(350), 2,
      sym_sampled_texture_type,
      sym_multisampled_texture_type,
    ACTIONS(61), 3,
      sym_vec2,
      sym_vec3,
      sym_vec4,
    ACTIONS(57), 4,
      sym_texture_storage_1d,
      sym_texture_storage_2d,
      sym_texture_storage_2d_array,
      sym_texture_storage_3d,
    ACTIONS(45), 5,
      sym_bool,
      sym_float32,
      sym_float16,
      sym_int32,
      sym_uint32,
    ACTIONS(59), 5,
      sym_texture_depth_2d,
      sym_texture_depth_2d_array,
      sym_texture_depth_cube,
      sym_texture_depth_cube_array,
      sym_texture_depth_multisampled_2d,
    ACTIONS(53), 6,
      sym_texture_1d,
      sym_texture_2d,
      sym_texture_2d_array,
      sym_texture_3d,
      sym_texture_cube,
      sym_texture_cube_array,
    ACTIONS(47), 9,
      sym_mat2x2,
      sym_mat2x3,
      sym_mat2x4,
      sym_mat3x2,
      sym_mat3x3,
      sym_mat3x4,
      sym_mat4x2,
      sym_mat4x3,
      sym_mat4x4,
  [1185] = 22,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(41), 1,
      sym_array,
    ACTIONS(43), 1,
      sym_atomic,
    ACTIONS(49), 1,
      sym_pointer,
    ACTIONS(55), 1,
      sym_texture_multisampled_2d,
    ACTIONS(186), 1,
      sym_ident,
    STATE(60), 1,
      sym__comment,
    STATE(212), 1,
      sym_type_decl_without_ident,
    STATE(296), 1,
      sym_type_decl,
    STATE(357), 1,
      sym_storage_texture_type,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(51), 2,
      sym_sampler,
      sym_sampler_comparison,
    STATE(199), 2,
      sym_array_type_decl,
      sym_texture_sampler_types,
    STATE(203), 2,
      sym_sampler_type,
      sym_depth_texture_type,
    STATE(325), 2,
      sym_vec_prefix,
      sym_mat_prefix,
    STATE(350), 2,
      sym_sampled_texture_type,
      sym_multisampled_texture_type,
    ACTIONS(61), 3,
      sym_vec2,
      sym_vec3,
      sym_vec4,
    ACTIONS(57), 4,
      sym_texture_storage_1d,
      sym_texture_storage_2d,
      sym_texture_storage_2d_array,
      sym_texture_storage_3d,
    ACTIONS(45), 5,
      sym_bool,
      sym_float32,
      sym_float16,
      sym_int32,
      sym_uint32,
    ACTIONS(59), 5,
      sym_texture_depth_2d,
      sym_texture_depth_2d_array,
      sym_texture_depth_cube,
      sym_texture_depth_cube_array,
      sym_texture_depth_multisampled_2d,
    ACTIONS(53), 6,
      sym_texture_1d,
      sym_texture_2d,
      sym_texture_2d_array,
      sym_texture_3d,
      sym_texture_cube,
      sym_texture_cube_array,
    ACTIONS(47), 9,
      sym_mat2x2,
      sym_mat2x3,
      sym_mat2x4,
      sym_mat3x2,
      sym_mat3x3,
      sym_mat3x4,
      sym_mat4x2,
      sym_mat4x3,
      sym_mat4x4,
  [1284] = 22,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(41), 1,
      sym_array,
    ACTIONS(43), 1,
      sym_atomic,
    ACTIONS(49), 1,
      sym_pointer,
    ACTIONS(55), 1,
      sym_texture_multisampled_2d,
    ACTIONS(186), 1,
      sym_ident,
    STATE(61), 1,
      sym__comment,
    STATE(212), 1,
      sym_type_decl_without_ident,
    STATE(357), 1,
      sym_storage_texture_type,
    STATE(414), 1,
      sym_type_decl,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(51), 2,
      sym_sampler,
      sym_sampler_comparison,
    STATE(199), 2,
      sym_array_type_decl,
      sym_texture_sampler_types,
    STATE(203), 2,
      sym_sampler_type,
      sym_depth_texture_type,
    STATE(325), 2,
      sym_vec_prefix,
      sym_mat_prefix,
    STATE(350), 2,
      sym_sampled_texture_type,
      sym_multisampled_texture_type,
    ACTIONS(61), 3,
      sym_vec2,
      sym_vec3,
      sym_vec4,
    ACTIONS(57), 4,
      sym_texture_storage_1d,
      sym_texture_storage_2d,
      sym_texture_storage_2d_array,
      sym_texture_storage_3d,
    ACTIONS(45), 5,
      sym_bool,
      sym_float32,
      sym_float16,
      sym_int32,
      sym_uint32,
    ACTIONS(59), 5,
      sym_texture_depth_2d,
      sym_texture_depth_2d_array,
      sym_texture_depth_cube,
      sym_texture_depth_cube_array,
      sym_texture_depth_multisampled_2d,
    ACTIONS(53), 6,
      sym_texture_1d,
      sym_texture_2d,
      sym_texture_2d_array,
      sym_texture_3d,
      sym_texture_cube,
      sym_texture_cube_array,
    ACTIONS(47), 9,
      sym_mat2x2,
      sym_mat2x3,
      sym_mat2x4,
      sym_mat3x2,
      sym_mat3x3,
      sym_mat3x4,
      sym_mat4x2,
      sym_mat4x3,
      sym_mat4x4,
  [1383] = 22,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(41), 1,
      sym_array,
    ACTIONS(43), 1,
      sym_atomic,
    ACTIONS(49), 1,
      sym_pointer,
    ACTIONS(55), 1,
      sym_texture_multisampled_2d,
    ACTIONS(186), 1,
      sym_ident,
    STATE(62), 1,
      sym__comment,
    STATE(212), 1,
      sym_type_decl_without_ident,
    STATE(357), 1,
      sym_storage_texture_type,
    STATE(387), 1,
      sym_type_decl,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(51), 2,
      sym_sampler,
      sym_sampler_comparison,
    STATE(199), 2,
      sym_array_type_decl,
      sym_texture_sampler_types,
    STATE(203), 2,
      sym_sampler_type,
      sym_depth_texture_type,
    STATE(325), 2,
      sym_vec_prefix,
      sym_mat_prefix,
    STATE(350), 2,
      sym_sampled_texture_type,
      sym_multisampled_texture_type,
    ACTIONS(61), 3,
      sym_vec2,
      sym_vec3,
      sym_vec4,
    ACTIONS(57), 4,
      sym_texture_storage_1d,
      sym_texture_storage_2d,
      sym_texture_storage_2d_array,
      sym_texture_storage_3d,
    ACTIONS(45), 5,
      sym_bool,
      sym_float32,
      sym_float16,
      sym_int32,
      sym_uint32,
    ACTIONS(59), 5,
      sym_texture_depth_2d,
      sym_texture_depth_2d_array,
      sym_texture_depth_cube,
      sym_texture_depth_cube_array,
      sym_texture_depth_multisampled_2d,
    ACTIONS(53), 6,
      sym_texture_1d,
      sym_texture_2d,
      sym_texture_2d_array,
      sym_texture_3d,
      sym_texture_cube,
      sym_texture_cube_array,
    ACTIONS(47), 9,
      sym_mat2x2,
      sym_mat2x3,
      sym_mat2x4,
      sym_mat3x2,
      sym_mat3x3,
      sym_mat3x4,
      sym_mat4x2,
      sym_mat4x3,
      sym_mat4x4,
  [1482] = 22,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(41), 1,
      sym_array,
    ACTIONS(43), 1,
      sym_atomic,
    ACTIONS(49), 1,
      sym_pointer,
    ACTIONS(55), 1,
      sym_texture_multisampled_2d,
    ACTIONS(186), 1,
      sym_ident,
    STATE(63), 1,
      sym__comment,
    STATE(212), 1,
      sym_type_decl_without_ident,
    STATE(271), 1,
      sym_type_decl,
    STATE(357), 1,
      sym_storage_texture_type,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(51), 2,
      sym_sampler,
      sym_sampler_comparison,
    STATE(199), 2,
      sym_array_type_decl,
      sym_texture_sampler_types,
    STATE(203), 2,
      sym_sampler_type,
      sym_depth_texture_type,
    STATE(325), 2,
      sym_vec_prefix,
      sym_mat_prefix,
    STATE(350), 2,
      sym_sampled_texture_type,
      sym_multisampled_texture_type,
    ACTIONS(61), 3,
      sym_vec2,
      sym_vec3,
      sym_vec4,
    ACTIONS(57), 4,
      sym_texture_storage_1d,
      sym_texture_storage_2d,
      sym_texture_storage_2d_array,
      sym_texture_storage_3d,
    ACTIONS(45), 5,
      sym_bool,
      sym_float32,
      sym_float16,
      sym_int32,
      sym_uint32,
    ACTIONS(59), 5,
      sym_texture_depth_2d,
      sym_texture_depth_2d_array,
      sym_texture_depth_cube,
      sym_texture_depth_cube_array,
      sym_texture_depth_multisampled_2d,
    ACTIONS(53), 6,
      sym_texture_1d,
      sym_texture_2d,
      sym_texture_2d_array,
      sym_texture_3d,
      sym_texture_cube,
      sym_texture_cube_array,
    ACTIONS(47), 9,
      sym_mat2x2,
      sym_mat2x3,
      sym_mat2x4,
      sym_mat3x2,
      sym_mat3x3,
      sym_mat3x4,
      sym_mat4x2,
      sym_mat4x3,
      sym_mat4x4,
  [1581] = 22,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(41), 1,
      sym_array,
    ACTIONS(43), 1,
      sym_atomic,
    ACTIONS(49), 1,
      sym_pointer,
    ACTIONS(55), 1,
      sym_texture_multisampled_2d,
    ACTIONS(186), 1,
      sym_ident,
    STATE(64), 1,
      sym__comment,
    STATE(212), 1,
      sym_type_decl_without_ident,
    STATE(357), 1,
      sym_storage_texture_type,
    STATE(393), 1,
      sym_type_decl,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(51), 2,
      sym_sampler,
      sym_sampler_comparison,
    STATE(199), 2,
      sym_array_type_decl,
      sym_texture_sampler_types,
    STATE(203), 2,
      sym_sampler_type,
      sym_depth_texture_type,
    STATE(325), 2,
      sym_vec_prefix,
      sym_mat_prefix,
    STATE(350), 2,
      sym_sampled_texture_type,
      sym_multisampled_texture_type,
    ACTIONS(61), 3,
      sym_vec2,
      sym_vec3,
      sym_vec4,
    ACTIONS(57), 4,
      sym_texture_storage_1d,
      sym_texture_storage_2d,
      sym_texture_storage_2d_array,
      sym_texture_storage_3d,
    ACTIONS(45), 5,
      sym_bool,
      sym_float32,
      sym_float16,
      sym_int32,
      sym_uint32,
    ACTIONS(59), 5,
      sym_texture_depth_2d,
      sym_texture_depth_2d_array,
      sym_texture_depth_cube,
      sym_texture_depth_cube_array,
      sym_texture_depth_multisampled_2d,
    ACTIONS(53), 6,
      sym_texture_1d,
      sym_texture_2d,
      sym_texture_2d_array,
      sym_texture_3d,
      sym_texture_cube,
      sym_texture_cube_array,
    ACTIONS(47), 9,
      sym_mat2x2,
      sym_mat2x3,
      sym_mat2x4,
      sym_mat3x2,
      sym_mat3x3,
      sym_mat3x4,
      sym_mat4x2,
      sym_mat4x3,
      sym_mat4x4,
  [1680] = 6,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(190), 1,
      sym_attr,
    STATE(75), 1,
      sym_attribute,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    STATE(65), 2,
      sym__comment,
      aux_sym_struct_member_repeat1,
    ACTIONS(188), 42,
      sym_array,
      sym_atomic,
      sym_bool,
      sym_float32,
      sym_float16,
      sym_int32,
      sym_mat2x2,
      sym_mat2x3,
      sym_mat2x4,
      sym_mat3x2,
      sym_mat3x3,
      sym_mat3x4,
      sym_mat4x2,
      sym_mat4x3,
      sym_mat4x4,
      sym_override,
      sym_pointer,
      sym_sampler,
      sym_sampler_comparison,
      sym_texture_1d,
      sym_texture_2d,
      sym_texture_2d_array,
      sym_texture_3d,
      sym_texture_cube,
      sym_texture_cube_array,
      sym_texture_multisampled_2d,
      sym_texture_storage_1d,
      sym_texture_storage_2d,
      sym_texture_storage_2d_array,
      sym_texture_storage_3d,
      sym_texture_depth_2d,
      sym_texture_depth_2d_array,
      sym_texture_depth_cube,
      sym_texture_depth_cube_array,
      sym_texture_depth_multisampled_2d,
      sym_uint32,
      sym_vec2,
      sym_vec3,
      sym_vec4,
      sym_fn,
      sym_var,
      sym_ident,
  [1742] = 30,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(193), 1,
      sym_ident,
    ACTIONS(196), 1,
      sym_staticAssert,
    ACTIONS(199), 1,
      sym_break,
    ACTIONS(205), 1,
      sym_continue,
    ACTIONS(210), 1,
      sym_discard,
    ACTIONS(213), 1,
      sym_for,
    ACTIONS(216), 1,
      sym_if,
    ACTIONS(219), 1,
      sym_loop,
    ACTIONS(222), 1,
      sym_return,
    ACTIONS(225), 1,
      sym_switch,
    ACTIONS(228), 1,
      sym_var,
    ACTIONS(231), 1,
      sym_while,
    ACTIONS(237), 1,
      sym_brace_left,
    ACTIONS(240), 1,
      sym_brace_right,
    ACTIONS(242), 1,
      sym_paren_left,
    ACTIONS(245), 1,
      sym_semicolon,
    ACTIONS(248), 1,
      sym_underscore,
    STATE(111), 1,
      sym_statement,
    STATE(159), 1,
      sym_core_lhs_expression,
    STATE(166), 1,
      sym_lhs_expression,
    STATE(222), 1,
      aux_sym_lhs_expression_repeat1,
    STATE(299), 1,
      sym_variable_decl,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(202), 2,
      sym_const,
      sym_let,
    ACTIONS(208), 2,
      sym_continuing,
      sym_fallthrough,
    ACTIONS(234), 2,
      sym_and,
      sym_star,
    STATE(66), 2,
      sym__comment,
      aux_sym_compound_statement_repeat1,
    STATE(108), 6,
      sym_compound_statement,
      sym_if_statement,
      sym_switch_statement,
      sym_loop_statement,
      sym_for_statement,
      sym_while_statement,
    STATE(408), 9,
      sym_variable_statement,
      sym_assignment_statement,
      sym_increment_statement,
      sym_decrement_statement,
      sym_break_statement,
      sym_continue_statement,
      sym_return_statement,
      sym_func_call_statement,
      sym_static_assert_statement,
  [1851] = 32,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(251), 1,
      sym_ident,
    ACTIONS(253), 1,
      sym_staticAssert,
    ACTIONS(255), 1,
      sym_break,
    ACTIONS(259), 1,
      sym_continue,
    ACTIONS(261), 1,
      sym_continuing,
    ACTIONS(263), 1,
      sym_discard,
    ACTIONS(265), 1,
      sym_for,
    ACTIONS(267), 1,
      sym_if,
    ACTIONS(269), 1,
      sym_loop,
    ACTIONS(271), 1,
      sym_return,
    ACTIONS(273), 1,
      sym_switch,
    ACTIONS(275), 1,
      sym_var,
    ACTIONS(277), 1,
      sym_while,
    ACTIONS(281), 1,
      sym_brace_left,
    ACTIONS(283), 1,
      sym_brace_right,
    ACTIONS(285), 1,
      sym_paren_left,
    ACTIONS(287), 1,
      sym_semicolon,
    ACTIONS(289), 1,
      sym_underscore,
    STATE(66), 1,
      aux_sym_compound_statement_repeat1,
    STATE(67), 1,
      sym__comment,
    STATE(111), 1,
      sym_statement,
    STATE(159), 1,
      sym_core_lhs_expression,
    STATE(166), 1,
      sym_lhs_expression,
    STATE(222), 1,
      aux_sym_lhs_expression_repeat1,
    STATE(299), 1,
      sym_variable_decl,
    STATE(324), 1,
      sym_continuing_statement,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(257), 2,
      sym_const,
      sym_let,
    ACTIONS(279), 2,
      sym_and,
      sym_star,
    STATE(108), 6,
      sym_compound_statement,
      sym_if_statement,
      sym_switch_statement,
      sym_loop_statement,
      sym_for_statement,
      sym_while_statement,
    STATE(408), 9,
      sym_variable_statement,
      sym_assignment_statement,
      sym_increment_statement,
      sym_decrement_statement,
      sym_break_statement,
      sym_continue_statement,
      sym_return_statement,
      sym_func_call_statement,
      sym_static_assert_statement,
  [1964] = 32,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(251), 1,
      sym_ident,
    ACTIONS(253), 1,
      sym_staticAssert,
    ACTIONS(255), 1,
      sym_break,
    ACTIONS(259), 1,
      sym_continue,
    ACTIONS(261), 1,
      sym_continuing,
    ACTIONS(263), 1,
      sym_discard,
    ACTIONS(265), 1,
      sym_for,
    ACTIONS(267), 1,
      sym_if,
    ACTIONS(269), 1,
      sym_loop,
    ACTIONS(271), 1,
      sym_return,
    ACTIONS(273), 1,
      sym_switch,
    ACTIONS(275), 1,
      sym_var,
    ACTIONS(277), 1,
      sym_while,
    ACTIONS(281), 1,
      sym_brace_left,
    ACTIONS(285), 1,
      sym_paren_left,
    ACTIONS(287), 1,
      sym_semicolon,
    ACTIONS(289), 1,
      sym_underscore,
    ACTIONS(291), 1,
      sym_brace_right,
    STATE(67), 1,
      aux_sym_compound_statement_repeat1,
    STATE(68), 1,
      sym__comment,
    STATE(111), 1,
      sym_statement,
    STATE(159), 1,
      sym_core_lhs_expression,
    STATE(166), 1,
      sym_lhs_expression,
    STATE(222), 1,
      aux_sym_lhs_expression_repeat1,
    STATE(299), 1,
      sym_variable_decl,
    STATE(363), 1,
      sym_continuing_statement,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(257), 2,
      sym_const,
      sym_let,
    ACTIONS(279), 2,
      sym_and,
      sym_star,
    STATE(108), 6,
      sym_compound_statement,
      sym_if_statement,
      sym_switch_statement,
      sym_loop_statement,
      sym_for_statement,
      sym_while_statement,
    STATE(408), 9,
      sym_variable_statement,
      sym_assignment_statement,
      sym_increment_statement,
      sym_decrement_statement,
      sym_break_statement,
      sym_continue_statement,
      sym_return_statement,
      sym_func_call_statement,
      sym_static_assert_statement,
  [2077] = 32,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(251), 1,
      sym_ident,
    ACTIONS(253), 1,
      sym_staticAssert,
    ACTIONS(255), 1,
      sym_break,
    ACTIONS(259), 1,
      sym_continue,
    ACTIONS(263), 1,
      sym_discard,
    ACTIONS(265), 1,
      sym_for,
    ACTIONS(267), 1,
      sym_if,
    ACTIONS(269), 1,
      sym_loop,
    ACTIONS(271), 1,
      sym_return,
    ACTIONS(273), 1,
      sym_switch,
    ACTIONS(275), 1,
      sym_var,
    ACTIONS(277), 1,
      sym_while,
    ACTIONS(281), 1,
      sym_brace_left,
    ACTIONS(285), 1,
      sym_paren_left,
    ACTIONS(287), 1,
      sym_semicolon,
    ACTIONS(289), 1,
      sym_underscore,
    ACTIONS(293), 1,
      sym_fallthrough,
    ACTIONS(295), 1,
      sym_brace_right,
    STATE(69), 1,
      sym__comment,
    STATE(71), 1,
      aux_sym_compound_statement_repeat1,
    STATE(111), 1,
      sym_statement,
    STATE(159), 1,
      sym_core_lhs_expression,
    STATE(166), 1,
      sym_lhs_expression,
    STATE(222), 1,
      aux_sym_lhs_expression_repeat1,
    STATE(299), 1,
      sym_variable_decl,
    STATE(343), 1,
      sym_fallthrough_statement,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(257), 2,
      sym_const,
      sym_let,
    ACTIONS(279), 2,
      sym_and,
      sym_star,
    STATE(108), 6,
      sym_compound_statement,
      sym_if_statement,
      sym_switch_statement,
      sym_loop_statement,
      sym_for_statement,
      sym_while_statement,
    STATE(408), 9,
      sym_variable_statement,
      sym_assignment_statement,
      sym_increment_statement,
      sym_decrement_statement,
      sym_break_statement,
      sym_continue_statement,
      sym_return_statement,
      sym_func_call_statement,
      sym_static_assert_statement,
  [2190] = 6,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(299), 1,
      sym_attr,
    ACTIONS(301), 1,
      sym_paren_left,
    STATE(70), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(297), 42,
      sym_array,
      sym_atomic,
      sym_bool,
      sym_float32,
      sym_float16,
      sym_int32,
      sym_mat2x2,
      sym_mat2x3,
      sym_mat2x4,
      sym_mat3x2,
      sym_mat3x3,
      sym_mat3x4,
      sym_mat4x2,
      sym_mat4x3,
      sym_mat4x4,
      sym_override,
      sym_pointer,
      sym_sampler,
      sym_sampler_comparison,
      sym_texture_1d,
      sym_texture_2d,
      sym_texture_2d_array,
      sym_texture_3d,
      sym_texture_cube,
      sym_texture_cube_array,
      sym_texture_multisampled_2d,
      sym_texture_storage_1d,
      sym_texture_storage_2d,
      sym_texture_storage_2d_array,
      sym_texture_storage_3d,
      sym_texture_depth_2d,
      sym_texture_depth_2d_array,
      sym_texture_depth_cube,
      sym_texture_depth_cube_array,
      sym_texture_depth_multisampled_2d,
      sym_uint32,
      sym_vec2,
      sym_vec3,
      sym_vec4,
      sym_fn,
      sym_var,
      sym_ident,
  [2251] = 32,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(251), 1,
      sym_ident,
    ACTIONS(253), 1,
      sym_staticAssert,
    ACTIONS(255), 1,
      sym_break,
    ACTIONS(259), 1,
      sym_continue,
    ACTIONS(263), 1,
      sym_discard,
    ACTIONS(265), 1,
      sym_for,
    ACTIONS(267), 1,
      sym_if,
    ACTIONS(269), 1,
      sym_loop,
    ACTIONS(271), 1,
      sym_return,
    ACTIONS(273), 1,
      sym_switch,
    ACTIONS(275), 1,
      sym_var,
    ACTIONS(277), 1,
      sym_while,
    ACTIONS(281), 1,
      sym_brace_left,
    ACTIONS(285), 1,
      sym_paren_left,
    ACTIONS(287), 1,
      sym_semicolon,
    ACTIONS(289), 1,
      sym_underscore,
    ACTIONS(293), 1,
      sym_fallthrough,
    ACTIONS(303), 1,
      sym_brace_right,
    STATE(66), 1,
      aux_sym_compound_statement_repeat1,
    STATE(71), 1,
      sym__comment,
    STATE(111), 1,
      sym_statement,
    STATE(159), 1,
      sym_core_lhs_expression,
    STATE(166), 1,
      sym_lhs_expression,
    STATE(222), 1,
      aux_sym_lhs_expression_repeat1,
    STATE(299), 1,
      sym_variable_decl,
    STATE(354), 1,
      sym_fallthrough_statement,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(257), 2,
      sym_const,
      sym_let,
    ACTIONS(279), 2,
      sym_and,
      sym_star,
    STATE(108), 6,
      sym_compound_statement,
      sym_if_statement,
      sym_switch_statement,
      sym_loop_statement,
      sym_for_statement,
      sym_while_statement,
    STATE(408), 9,
      sym_variable_statement,
      sym_assignment_statement,
      sym_increment_statement,
      sym_decrement_statement,
      sym_break_statement,
      sym_continue_statement,
      sym_return_statement,
      sym_func_call_statement,
      sym_static_assert_statement,
  [2364] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(307), 1,
      sym_attr,
    STATE(72), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(305), 42,
      sym_array,
      sym_atomic,
      sym_bool,
      sym_float32,
      sym_float16,
      sym_int32,
      sym_mat2x2,
      sym_mat2x3,
      sym_mat2x4,
      sym_mat3x2,
      sym_mat3x3,
      sym_mat3x4,
      sym_mat4x2,
      sym_mat4x3,
      sym_mat4x4,
      sym_override,
      sym_pointer,
      sym_sampler,
      sym_sampler_comparison,
      sym_texture_1d,
      sym_texture_2d,
      sym_texture_2d_array,
      sym_texture_3d,
      sym_texture_cube,
      sym_texture_cube_array,
      sym_texture_multisampled_2d,
      sym_texture_storage_1d,
      sym_texture_storage_2d,
      sym_texture_storage_2d_array,
      sym_texture_storage_3d,
      sym_texture_depth_2d,
      sym_texture_depth_2d_array,
      sym_texture_depth_cube,
      sym_texture_depth_cube_array,
      sym_texture_depth_multisampled_2d,
      sym_uint32,
      sym_vec2,
      sym_vec3,
      sym_vec4,
      sym_fn,
      sym_var,
      sym_ident,
  [2422] = 31,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(251), 1,
      sym_ident,
    ACTIONS(253), 1,
      sym_staticAssert,
    ACTIONS(259), 1,
      sym_continue,
    ACTIONS(263), 1,
      sym_discard,
    ACTIONS(265), 1,
      sym_for,
    ACTIONS(267), 1,
      sym_if,
    ACTIONS(269), 1,
      sym_loop,
    ACTIONS(271), 1,
      sym_return,
    ACTIONS(273), 1,
      sym_switch,
    ACTIONS(275), 1,
      sym_var,
    ACTIONS(277), 1,
      sym_while,
    ACTIONS(281), 1,
      sym_brace_left,
    ACTIONS(285), 1,
      sym_paren_left,
    ACTIONS(287), 1,
      sym_semicolon,
    ACTIONS(289), 1,
      sym_underscore,
    ACTIONS(309), 1,
      sym_break,
    ACTIONS(311), 1,
      sym_brace_right,
    STATE(66), 1,
      aux_sym_compound_statement_repeat1,
    STATE(73), 1,
      sym__comment,
    STATE(111), 1,
      sym_statement,
    STATE(159), 1,
      sym_core_lhs_expression,
    STATE(166), 1,
      sym_lhs_expression,
    STATE(222), 1,
      aux_sym_lhs_expression_repeat1,
    STATE(299), 1,
      sym_variable_decl,
    STATE(336), 1,
      sym_break_if_statement,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(257), 2,
      sym_const,
      sym_let,
    ACTIONS(279), 2,
      sym_and,
      sym_star,
    STATE(108), 6,
      sym_compound_statement,
      sym_if_statement,
      sym_switch_statement,
      sym_loop_statement,
      sym_for_statement,
      sym_while_statement,
    STATE(408), 9,
      sym_variable_statement,
      sym_assignment_statement,
      sym_increment_statement,
      sym_decrement_statement,
      sym_break_statement,
      sym_continue_statement,
      sym_return_statement,
      sym_func_call_statement,
      sym_static_assert_statement,
  [2532] = 31,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(251), 1,
      sym_ident,
    ACTIONS(253), 1,
      sym_staticAssert,
    ACTIONS(259), 1,
      sym_continue,
    ACTIONS(263), 1,
      sym_discard,
    ACTIONS(265), 1,
      sym_for,
    ACTIONS(267), 1,
      sym_if,
    ACTIONS(269), 1,
      sym_loop,
    ACTIONS(271), 1,
      sym_return,
    ACTIONS(273), 1,
      sym_switch,
    ACTIONS(275), 1,
      sym_var,
    ACTIONS(277), 1,
      sym_while,
    ACTIONS(281), 1,
      sym_brace_left,
    ACTIONS(285), 1,
      sym_paren_left,
    ACTIONS(287), 1,
      sym_semicolon,
    ACTIONS(289), 1,
      sym_underscore,
    ACTIONS(309), 1,
      sym_break,
    ACTIONS(313), 1,
      sym_brace_right,
    STATE(73), 1,
      aux_sym_compound_statement_repeat1,
    STATE(74), 1,
      sym__comment,
    STATE(111), 1,
      sym_statement,
    STATE(159), 1,
      sym_core_lhs_expression,
    STATE(166), 1,
      sym_lhs_expression,
    STATE(222), 1,
      aux_sym_lhs_expression_repeat1,
    STATE(299), 1,
      sym_variable_decl,
    STATE(320), 1,
      sym_break_if_statement,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(257), 2,
      sym_const,
      sym_let,
    ACTIONS(279), 2,
      sym_and,
      sym_star,
    STATE(108), 6,
      sym_compound_statement,
      sym_if_statement,
      sym_switch_statement,
      sym_loop_statement,
      sym_for_statement,
      sym_while_statement,
    STATE(408), 9,
      sym_variable_statement,
      sym_assignment_statement,
      sym_increment_statement,
      sym_decrement_statement,
      sym_break_statement,
      sym_continue_statement,
      sym_return_statement,
      sym_func_call_statement,
      sym_static_assert_statement,
  [2642] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(317), 1,
      sym_attr,
    STATE(75), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(315), 42,
      sym_array,
      sym_atomic,
      sym_bool,
      sym_float32,
      sym_float16,
      sym_int32,
      sym_mat2x2,
      sym_mat2x3,
      sym_mat2x4,
      sym_mat3x2,
      sym_mat3x3,
      sym_mat3x4,
      sym_mat4x2,
      sym_mat4x3,
      sym_mat4x4,
      sym_override,
      sym_pointer,
      sym_sampler,
      sym_sampler_comparison,
      sym_texture_1d,
      sym_texture_2d,
      sym_texture_2d_array,
      sym_texture_3d,
      sym_texture_cube,
      sym_texture_cube_array,
      sym_texture_multisampled_2d,
      sym_texture_storage_1d,
      sym_texture_storage_2d,
      sym_texture_storage_2d_array,
      sym_texture_storage_3d,
      sym_texture_depth_2d,
      sym_texture_depth_2d_array,
      sym_texture_depth_cube,
      sym_texture_depth_cube_array,
      sym_texture_depth_multisampled_2d,
      sym_uint32,
      sym_vec2,
      sym_vec3,
      sym_vec4,
      sym_fn,
      sym_var,
      sym_ident,
  [2700] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(321), 1,
      sym_attr,
    STATE(76), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(319), 42,
      sym_array,
      sym_atomic,
      sym_bool,
      sym_float32,
      sym_float16,
      sym_int32,
      sym_mat2x2,
      sym_mat2x3,
      sym_mat2x4,
      sym_mat3x2,
      sym_mat3x3,
      sym_mat3x4,
      sym_mat4x2,
      sym_mat4x3,
      sym_mat4x4,
      sym_override,
      sym_pointer,
      sym_sampler,
      sym_sampler_comparison,
      sym_texture_1d,
      sym_texture_2d,
      sym_texture_2d_array,
      sym_texture_3d,
      sym_texture_cube,
      sym_texture_cube_array,
      sym_texture_multisampled_2d,
      sym_texture_storage_1d,
      sym_texture_storage_2d,
      sym_texture_storage_2d_array,
      sym_texture_storage_3d,
      sym_texture_depth_2d,
      sym_texture_depth_2d_array,
      sym_texture_depth_cube,
      sym_texture_depth_cube_array,
      sym_texture_depth_multisampled_2d,
      sym_uint32,
      sym_vec2,
      sym_vec3,
      sym_vec4,
      sym_fn,
      sym_var,
      sym_ident,
  [2758] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(325), 1,
      sym_attr,
    STATE(77), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(323), 42,
      sym_array,
      sym_atomic,
      sym_bool,
      sym_float32,
      sym_float16,
      sym_int32,
      sym_mat2x2,
      sym_mat2x3,
      sym_mat2x4,
      sym_mat3x2,
      sym_mat3x3,
      sym_mat3x4,
      sym_mat4x2,
      sym_mat4x3,
      sym_mat4x4,
      sym_override,
      sym_pointer,
      sym_sampler,
      sym_sampler_comparison,
      sym_texture_1d,
      sym_texture_2d,
      sym_texture_2d_array,
      sym_texture_3d,
      sym_texture_cube,
      sym_texture_cube_array,
      sym_texture_multisampled_2d,
      sym_texture_storage_1d,
      sym_texture_storage_2d,
      sym_texture_storage_2d_array,
      sym_texture_storage_3d,
      sym_texture_depth_2d,
      sym_texture_depth_2d_array,
      sym_texture_depth_cube,
      sym_texture_depth_cube_array,
      sym_texture_depth_multisampled_2d,
      sym_uint32,
      sym_vec2,
      sym_vec3,
      sym_vec4,
      sym_fn,
      sym_var,
      sym_ident,
  [2816] = 30,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(251), 1,
      sym_ident,
    ACTIONS(253), 1,
      sym_staticAssert,
    ACTIONS(255), 1,
      sym_break,
    ACTIONS(259), 1,
      sym_continue,
    ACTIONS(263), 1,
      sym_discard,
    ACTIONS(265), 1,
      sym_for,
    ACTIONS(267), 1,
      sym_if,
    ACTIONS(269), 1,
      sym_loop,
    ACTIONS(271), 1,
      sym_return,
    ACTIONS(273), 1,
      sym_switch,
    ACTIONS(275), 1,
      sym_var,
    ACTIONS(277), 1,
      sym_while,
    ACTIONS(281), 1,
      sym_brace_left,
    ACTIONS(285), 1,
      sym_paren_left,
    ACTIONS(287), 1,
      sym_semicolon,
    ACTIONS(289), 1,
      sym_underscore,
    ACTIONS(327), 1,
      sym_brace_right,
    STATE(66), 1,
      aux_sym_compound_statement_repeat1,
    STATE(78), 1,
      sym__comment,
    STATE(111), 1,
      sym_statement,
    STATE(159), 1,
      sym_core_lhs_expression,
    STATE(166), 1,
      sym_lhs_expression,
    STATE(222), 1,
      aux_sym_lhs_expression_repeat1,
    STATE(299), 1,
      sym_variable_decl,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(257), 2,
      sym_const,
      sym_let,
    ACTIONS(279), 2,
      sym_and,
      sym_star,
    STATE(108), 6,
      sym_compound_statement,
      sym_if_statement,
      sym_switch_statement,
      sym_loop_statement,
      sym_for_statement,
      sym_while_statement,
    STATE(408), 9,
      sym_variable_statement,
      sym_assignment_statement,
      sym_increment_statement,
      sym_decrement_statement,
      sym_break_statement,
      sym_continue_statement,
      sym_return_statement,
      sym_func_call_statement,
      sym_static_assert_statement,
  [2923] = 30,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(251), 1,
      sym_ident,
    ACTIONS(253), 1,
      sym_staticAssert,
    ACTIONS(255), 1,
      sym_break,
    ACTIONS(259), 1,
      sym_continue,
    ACTIONS(263), 1,
      sym_discard,
    ACTIONS(265), 1,
      sym_for,
    ACTIONS(267), 1,
      sym_if,
    ACTIONS(269), 1,
      sym_loop,
    ACTIONS(271), 1,
      sym_return,
    ACTIONS(273), 1,
      sym_switch,
    ACTIONS(275), 1,
      sym_var,
    ACTIONS(277), 1,
      sym_while,
    ACTIONS(281), 1,
      sym_brace_left,
    ACTIONS(285), 1,
      sym_paren_left,
    ACTIONS(287), 1,
      sym_semicolon,
    ACTIONS(289), 1,
      sym_underscore,
    ACTIONS(329), 1,
      sym_brace_right,
    STATE(78), 1,
      aux_sym_compound_statement_repeat1,
    STATE(79), 1,
      sym__comment,
    STATE(111), 1,
      sym_statement,
    STATE(159), 1,
      sym_core_lhs_expression,
    STATE(166), 1,
      sym_lhs_expression,
    STATE(222), 1,
      aux_sym_lhs_expression_repeat1,
    STATE(299), 1,
      sym_variable_decl,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(257), 2,
      sym_const,
      sym_let,
    ACTIONS(279), 2,
      sym_and,
      sym_star,
    STATE(108), 6,
      sym_compound_statement,
      sym_if_statement,
      sym_switch_statement,
      sym_loop_statement,
      sym_for_statement,
      sym_while_statement,
    STATE(408), 9,
      sym_variable_statement,
      sym_assignment_statement,
      sym_increment_statement,
      sym_decrement_statement,
      sym_break_statement,
      sym_continue_statement,
      sym_return_statement,
      sym_func_call_statement,
      sym_static_assert_statement,
  [3030] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(80), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(331), 8,
      ts_builtin_sym_end,
      sym_and,
      sym_attr,
      sym_brace_left,
      sym_brace_right,
      sym_paren_left,
      sym_semicolon,
      sym_star,
    ACTIONS(333), 22,
      sym_override,
      sym_staticAssert,
      sym_struct,
      sym_break,
      sym_const,
      sym_continue,
      sym_continuing,
      sym_discard,
      sym_else,
      sym_fallthrough,
      sym_fn,
      sym_for,
      sym_if,
      sym_let,
      sym_loop,
      sym_return,
      sym_switch,
      sym_type,
      sym_var,
      sym_while,
      sym_underscore,
      sym_ident,
  [3075] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(81), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(335), 8,
      ts_builtin_sym_end,
      sym_and,
      sym_attr,
      sym_brace_left,
      sym_brace_right,
      sym_paren_left,
      sym_semicolon,
      sym_star,
    ACTIONS(337), 22,
      sym_override,
      sym_staticAssert,
      sym_struct,
      sym_break,
      sym_const,
      sym_continue,
      sym_continuing,
      sym_discard,
      sym_else,
      sym_fallthrough,
      sym_fn,
      sym_for,
      sym_if,
      sym_let,
      sym_loop,
      sym_return,
      sym_switch,
      sym_type,
      sym_var,
      sym_while,
      sym_underscore,
      sym_ident,
  [3120] = 8,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(343), 1,
      sym_bracket_left,
    ACTIONS(345), 1,
      sym_period,
    STATE(82), 1,
      sym__comment,
    STATE(117), 1,
      sym_postfix_expression,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(339), 5,
      sym_and,
      sym_forward_slash,
      sym_greater_than,
      sym_less_than,
      sym_or,
    ACTIONS(341), 17,
      sym_and_and,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
      sym_modulo,
      sym_minus,
      sym_plus,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [3166] = 8,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(343), 1,
      sym_bracket_left,
    ACTIONS(345), 1,
      sym_period,
    STATE(83), 1,
      sym__comment,
    STATE(115), 1,
      sym_postfix_expression,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(347), 5,
      sym_and,
      sym_forward_slash,
      sym_greater_than,
      sym_less_than,
      sym_or,
    ACTIONS(349), 17,
      sym_and_and,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
      sym_modulo,
      sym_minus,
      sym_plus,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [3212] = 8,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(343), 1,
      sym_bracket_left,
    ACTIONS(345), 1,
      sym_period,
    STATE(84), 1,
      sym__comment,
    STATE(120), 1,
      sym_postfix_expression,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(351), 5,
      sym_and,
      sym_forward_slash,
      sym_greater_than,
      sym_less_than,
      sym_or,
    ACTIONS(353), 17,
      sym_and_and,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
      sym_modulo,
      sym_minus,
      sym_plus,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [3258] = 24,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(9), 1,
      sym_override,
    ACTIONS(11), 1,
      sym_staticAssert,
    ACTIONS(13), 1,
      sym_struct,
    ACTIONS(15), 1,
      sym_const,
    ACTIONS(17), 1,
      sym_enable,
    ACTIONS(19), 1,
      sym_fn,
    ACTIONS(21), 1,
      sym_type,
    ACTIONS(23), 1,
      sym_var,
    ACTIONS(25), 1,
      sym_attr,
    ACTIONS(27), 1,
      sym_semicolon,
    ACTIONS(355), 1,
      ts_builtin_sym_end,
    STATE(75), 1,
      sym_attribute,
    STATE(85), 1,
      sym__comment,
    STATE(125), 1,
      aux_sym_translation_unit_repeat2,
    STATE(158), 1,
      aux_sym_translation_unit_repeat1,
    STATE(173), 1,
      sym_global_directive,
    STATE(175), 1,
      sym_enable_directive,
    STATE(190), 1,
      aux_sym_struct_member_repeat1,
    STATE(260), 1,
      sym_variable_decl,
    STATE(263), 1,
      sym_function_header,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    STATE(176), 2,
      sym_struct_decl,
      sym_function_decl,
    STATE(377), 4,
      sym_type_alias_decl,
      sym_global_variable_decl,
      sym_global_constant_decl,
      sym_static_assert_statement,
  [3336] = 6,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(361), 1,
      sym_paren_left,
    STATE(86), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(357), 5,
      sym_and,
      sym_forward_slash,
      sym_greater_than,
      sym_less_than,
      sym_or,
    ACTIONS(359), 19,
      sym_and_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [3378] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(87), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(363), 5,
      sym_and,
      sym_forward_slash,
      sym_greater_than,
      sym_less_than,
      sym_or,
    ACTIONS(365), 19,
      sym_and_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [3417] = 8,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(367), 1,
      sym_bracket_left,
    ACTIONS(369), 1,
      sym_period,
    STATE(88), 1,
      sym__comment,
    STATE(122), 1,
      sym_postfix_expression,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(347), 3,
      sym_and,
      sym_or,
      sym_xor,
    ACTIONS(349), 18,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal,
      sym_greater_than,
      sym_minus_minus,
      sym_plus_plus,
      sym_paren_right,
      sym_semicolon,
      sym_plus_equal,
      sym_minus_equal,
      sym_times_equal,
      sym_division_equal,
      sym_modulo_equal,
      sym_and_equal,
      sym_or_equal,
      sym_xor_equal,
  [3462] = 6,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(373), 1,
      sym_else,
    STATE(89), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(375), 6,
      sym_and,
      sym_brace_left,
      sym_brace_right,
      sym_paren_left,
      sym_semicolon,
      sym_star,
    ACTIONS(371), 17,
      sym_staticAssert,
      sym_break,
      sym_const,
      sym_continue,
      sym_continuing,
      sym_discard,
      sym_fallthrough,
      sym_for,
      sym_if,
      sym_let,
      sym_loop,
      sym_return,
      sym_switch,
      sym_var,
      sym_while,
      sym_underscore,
      sym_ident,
  [3503] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(90), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(377), 5,
      sym_and,
      sym_forward_slash,
      sym_greater_than,
      sym_less_than,
      sym_or,
    ACTIONS(379), 19,
      sym_and_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [3542] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(91), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(381), 5,
      sym_and,
      sym_forward_slash,
      sym_greater_than,
      sym_less_than,
      sym_or,
    ACTIONS(383), 19,
      sym_and_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [3581] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(92), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(357), 5,
      sym_and,
      sym_forward_slash,
      sym_greater_than,
      sym_less_than,
      sym_or,
    ACTIONS(359), 19,
      sym_and_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [3620] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(93), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(385), 5,
      sym_and,
      sym_forward_slash,
      sym_greater_than,
      sym_less_than,
      sym_or,
    ACTIONS(387), 19,
      sym_and_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [3659] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(94), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(389), 5,
      sym_and,
      sym_forward_slash,
      sym_greater_than,
      sym_less_than,
      sym_or,
    ACTIONS(391), 19,
      sym_and_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [3698] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(95), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(393), 5,
      sym_and,
      sym_forward_slash,
      sym_greater_than,
      sym_less_than,
      sym_or,
    ACTIONS(395), 19,
      sym_and_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [3737] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(96), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(397), 5,
      sym_and,
      sym_forward_slash,
      sym_greater_than,
      sym_less_than,
      sym_or,
    ACTIONS(399), 19,
      sym_and_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [3776] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(97), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(401), 5,
      sym_and,
      sym_forward_slash,
      sym_greater_than,
      sym_less_than,
      sym_or,
    ACTIONS(403), 19,
      sym_and_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [3815] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(98), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(405), 5,
      sym_and,
      sym_forward_slash,
      sym_greater_than,
      sym_less_than,
      sym_or,
    ACTIONS(407), 19,
      sym_and_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [3854] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(99), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(409), 5,
      sym_and,
      sym_forward_slash,
      sym_greater_than,
      sym_less_than,
      sym_or,
    ACTIONS(411), 19,
      sym_and_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [3893] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(100), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(413), 5,
      sym_and,
      sym_forward_slash,
      sym_greater_than,
      sym_less_than,
      sym_or,
    ACTIONS(415), 19,
      sym_and_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [3932] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(101), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(417), 5,
      sym_and,
      sym_forward_slash,
      sym_greater_than,
      sym_less_than,
      sym_or,
    ACTIONS(419), 19,
      sym_and_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [3971] = 8,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(367), 1,
      sym_bracket_left,
    ACTIONS(369), 1,
      sym_period,
    STATE(102), 1,
      sym__comment,
    STATE(121), 1,
      sym_postfix_expression,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(351), 3,
      sym_and,
      sym_or,
      sym_xor,
    ACTIONS(353), 18,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal,
      sym_greater_than,
      sym_minus_minus,
      sym_plus_plus,
      sym_paren_right,
      sym_semicolon,
      sym_plus_equal,
      sym_minus_equal,
      sym_times_equal,
      sym_division_equal,
      sym_modulo_equal,
      sym_and_equal,
      sym_or_equal,
      sym_xor_equal,
  [4016] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(103), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(421), 5,
      sym_and,
      sym_forward_slash,
      sym_greater_than,
      sym_less_than,
      sym_or,
    ACTIONS(423), 19,
      sym_and_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [4055] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(104), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(427), 6,
      sym_and,
      sym_brace_left,
      sym_brace_right,
      sym_paren_left,
      sym_semicolon,
      sym_star,
    ACTIONS(425), 17,
      sym_staticAssert,
      sym_break,
      sym_const,
      sym_continue,
      sym_continuing,
      sym_discard,
      sym_fallthrough,
      sym_for,
      sym_if,
      sym_let,
      sym_loop,
      sym_return,
      sym_switch,
      sym_var,
      sym_while,
      sym_underscore,
      sym_ident,
  [4093] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(105), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(431), 6,
      sym_and,
      sym_brace_left,
      sym_brace_right,
      sym_paren_left,
      sym_semicolon,
      sym_star,
    ACTIONS(429), 17,
      sym_staticAssert,
      sym_break,
      sym_const,
      sym_continue,
      sym_continuing,
      sym_discard,
      sym_fallthrough,
      sym_for,
      sym_if,
      sym_let,
      sym_loop,
      sym_return,
      sym_switch,
      sym_var,
      sym_while,
      sym_underscore,
      sym_ident,
  [4131] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(106), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(435), 6,
      sym_and,
      sym_brace_left,
      sym_brace_right,
      sym_paren_left,
      sym_semicolon,
      sym_star,
    ACTIONS(433), 17,
      sym_staticAssert,
      sym_break,
      sym_const,
      sym_continue,
      sym_continuing,
      sym_discard,
      sym_fallthrough,
      sym_for,
      sym_if,
      sym_let,
      sym_loop,
      sym_return,
      sym_switch,
      sym_var,
      sym_while,
      sym_underscore,
      sym_ident,
  [4169] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(107), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(439), 6,
      sym_and,
      sym_brace_left,
      sym_brace_right,
      sym_paren_left,
      sym_semicolon,
      sym_star,
    ACTIONS(437), 17,
      sym_staticAssert,
      sym_break,
      sym_const,
      sym_continue,
      sym_continuing,
      sym_discard,
      sym_fallthrough,
      sym_for,
      sym_if,
      sym_let,
      sym_loop,
      sym_return,
      sym_switch,
      sym_var,
      sym_while,
      sym_underscore,
      sym_ident,
  [4207] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(108), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(443), 6,
      sym_and,
      sym_brace_left,
      sym_brace_right,
      sym_paren_left,
      sym_semicolon,
      sym_star,
    ACTIONS(441), 17,
      sym_staticAssert,
      sym_break,
      sym_const,
      sym_continue,
      sym_continuing,
      sym_discard,
      sym_fallthrough,
      sym_for,
      sym_if,
      sym_let,
      sym_loop,
      sym_return,
      sym_switch,
      sym_var,
      sym_while,
      sym_underscore,
      sym_ident,
  [4245] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(109), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(447), 6,
      sym_and,
      sym_brace_left,
      sym_brace_right,
      sym_paren_left,
      sym_semicolon,
      sym_star,
    ACTIONS(445), 17,
      sym_staticAssert,
      sym_break,
      sym_const,
      sym_continue,
      sym_continuing,
      sym_discard,
      sym_fallthrough,
      sym_for,
      sym_if,
      sym_let,
      sym_loop,
      sym_return,
      sym_switch,
      sym_var,
      sym_while,
      sym_underscore,
      sym_ident,
  [4283] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(110), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(451), 6,
      sym_and,
      sym_brace_left,
      sym_brace_right,
      sym_paren_left,
      sym_semicolon,
      sym_star,
    ACTIONS(449), 17,
      sym_staticAssert,
      sym_break,
      sym_const,
      sym_continue,
      sym_continuing,
      sym_discard,
      sym_fallthrough,
      sym_for,
      sym_if,
      sym_let,
      sym_loop,
      sym_return,
      sym_switch,
      sym_var,
      sym_while,
      sym_underscore,
      sym_ident,
  [4321] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(111), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(455), 6,
      sym_and,
      sym_brace_left,
      sym_brace_right,
      sym_paren_left,
      sym_semicolon,
      sym_star,
    ACTIONS(453), 17,
      sym_staticAssert,
      sym_break,
      sym_const,
      sym_continue,
      sym_continuing,
      sym_discard,
      sym_fallthrough,
      sym_for,
      sym_if,
      sym_let,
      sym_loop,
      sym_return,
      sym_switch,
      sym_var,
      sym_while,
      sym_underscore,
      sym_ident,
  [4359] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(112), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(459), 6,
      sym_and,
      sym_brace_left,
      sym_brace_right,
      sym_paren_left,
      sym_semicolon,
      sym_star,
    ACTIONS(457), 17,
      sym_staticAssert,
      sym_break,
      sym_const,
      sym_continue,
      sym_continuing,
      sym_discard,
      sym_fallthrough,
      sym_for,
      sym_if,
      sym_let,
      sym_loop,
      sym_return,
      sym_switch,
      sym_var,
      sym_while,
      sym_underscore,
      sym_ident,
  [4397] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(113), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(463), 6,
      sym_and,
      sym_brace_left,
      sym_brace_right,
      sym_paren_left,
      sym_semicolon,
      sym_star,
    ACTIONS(461), 17,
      sym_staticAssert,
      sym_break,
      sym_const,
      sym_continue,
      sym_continuing,
      sym_discard,
      sym_fallthrough,
      sym_for,
      sym_if,
      sym_let,
      sym_loop,
      sym_return,
      sym_switch,
      sym_var,
      sym_while,
      sym_underscore,
      sym_ident,
  [4435] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(114), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(467), 6,
      sym_and,
      sym_brace_left,
      sym_brace_right,
      sym_paren_left,
      sym_semicolon,
      sym_star,
    ACTIONS(465), 17,
      sym_staticAssert,
      sym_break,
      sym_const,
      sym_continue,
      sym_continuing,
      sym_discard,
      sym_fallthrough,
      sym_for,
      sym_if,
      sym_let,
      sym_loop,
      sym_return,
      sym_switch,
      sym_var,
      sym_while,
      sym_underscore,
      sym_ident,
  [4473] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(115), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(469), 5,
      sym_and,
      sym_forward_slash,
      sym_greater_than,
      sym_less_than,
      sym_or,
    ACTIONS(471), 17,
      sym_and_and,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
      sym_modulo,
      sym_minus,
      sym_plus,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [4510] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(116), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(473), 5,
      sym_and,
      sym_forward_slash,
      sym_greater_than,
      sym_less_than,
      sym_or,
    ACTIONS(475), 17,
      sym_and_and,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
      sym_modulo,
      sym_minus,
      sym_plus,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [4547] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(117), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(477), 5,
      sym_and,
      sym_forward_slash,
      sym_greater_than,
      sym_less_than,
      sym_or,
    ACTIONS(479), 17,
      sym_and_and,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
      sym_modulo,
      sym_minus,
      sym_plus,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [4584] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(118), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(481), 5,
      sym_and,
      sym_forward_slash,
      sym_greater_than,
      sym_less_than,
      sym_or,
    ACTIONS(483), 17,
      sym_and_and,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
      sym_modulo,
      sym_minus,
      sym_plus,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [4621] = 8,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(485), 1,
      sym_and,
    ACTIONS(491), 1,
      sym_or,
    ACTIONS(493), 1,
      sym_xor,
    STATE(119), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(489), 3,
      sym_forward_slash,
      sym_greater_than,
      sym_less_than,
    ACTIONS(487), 16,
      sym_and_and,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
      sym_modulo,
      sym_minus,
      sym_plus,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
  [4664] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(120), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(347), 5,
      sym_and,
      sym_forward_slash,
      sym_greater_than,
      sym_less_than,
      sym_or,
    ACTIONS(349), 17,
      sym_and_and,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
      sym_modulo,
      sym_minus,
      sym_plus,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [4701] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(121), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(347), 3,
      sym_and,
      sym_or,
      sym_xor,
    ACTIONS(349), 18,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal,
      sym_greater_than,
      sym_minus_minus,
      sym_plus_plus,
      sym_paren_right,
      sym_semicolon,
      sym_plus_equal,
      sym_minus_equal,
      sym_times_equal,
      sym_division_equal,
      sym_modulo_equal,
      sym_and_equal,
      sym_or_equal,
      sym_xor_equal,
  [4737] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(122), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(469), 3,
      sym_and,
      sym_or,
      sym_xor,
    ACTIONS(471), 18,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal,
      sym_greater_than,
      sym_minus_minus,
      sym_plus_plus,
      sym_paren_right,
      sym_semicolon,
      sym_plus_equal,
      sym_minus_equal,
      sym_times_equal,
      sym_division_equal,
      sym_modulo_equal,
      sym_and_equal,
      sym_or_equal,
      sym_xor_equal,
  [4773] = 19,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(495), 1,
      ts_builtin_sym_end,
    ACTIONS(497), 1,
      sym_override,
    ACTIONS(500), 1,
      sym_staticAssert,
    ACTIONS(503), 1,
      sym_struct,
    ACTIONS(506), 1,
      sym_const,
    ACTIONS(509), 1,
      sym_fn,
    ACTIONS(512), 1,
      sym_type,
    ACTIONS(515), 1,
      sym_var,
    ACTIONS(518), 1,
      sym_attr,
    ACTIONS(521), 1,
      sym_semicolon,
    STATE(75), 1,
      sym_attribute,
    STATE(190), 1,
      aux_sym_struct_member_repeat1,
    STATE(260), 1,
      sym_variable_decl,
    STATE(263), 1,
      sym_function_header,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    STATE(123), 2,
      sym__comment,
      aux_sym_translation_unit_repeat2,
    STATE(176), 2,
      sym_struct_decl,
      sym_function_decl,
    STATE(377), 4,
      sym_type_alias_decl,
      sym_global_variable_decl,
      sym_global_constant_decl,
      sym_static_assert_statement,
  [4837] = 20,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(9), 1,
      sym_override,
    ACTIONS(11), 1,
      sym_staticAssert,
    ACTIONS(13), 1,
      sym_struct,
    ACTIONS(15), 1,
      sym_const,
    ACTIONS(19), 1,
      sym_fn,
    ACTIONS(21), 1,
      sym_type,
    ACTIONS(23), 1,
      sym_var,
    ACTIONS(25), 1,
      sym_attr,
    ACTIONS(27), 1,
      sym_semicolon,
    ACTIONS(355), 1,
      ts_builtin_sym_end,
    STATE(75), 1,
      sym_attribute,
    STATE(123), 1,
      aux_sym_translation_unit_repeat2,
    STATE(124), 1,
      sym__comment,
    STATE(190), 1,
      aux_sym_struct_member_repeat1,
    STATE(260), 1,
      sym_variable_decl,
    STATE(263), 1,
      sym_function_header,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    STATE(176), 2,
      sym_struct_decl,
      sym_function_decl,
    STATE(377), 4,
      sym_type_alias_decl,
      sym_global_variable_decl,
      sym_global_constant_decl,
      sym_static_assert_statement,
  [4903] = 20,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(9), 1,
      sym_override,
    ACTIONS(11), 1,
      sym_staticAssert,
    ACTIONS(13), 1,
      sym_struct,
    ACTIONS(15), 1,
      sym_const,
    ACTIONS(19), 1,
      sym_fn,
    ACTIONS(21), 1,
      sym_type,
    ACTIONS(23), 1,
      sym_var,
    ACTIONS(25), 1,
      sym_attr,
    ACTIONS(27), 1,
      sym_semicolon,
    ACTIONS(524), 1,
      ts_builtin_sym_end,
    STATE(75), 1,
      sym_attribute,
    STATE(123), 1,
      aux_sym_translation_unit_repeat2,
    STATE(125), 1,
      sym__comment,
    STATE(190), 1,
      aux_sym_struct_member_repeat1,
    STATE(260), 1,
      sym_variable_decl,
    STATE(263), 1,
      sym_function_header,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    STATE(176), 2,
      sym_struct_decl,
      sym_function_decl,
    STATE(377), 4,
      sym_type_alias_decl,
      sym_global_variable_decl,
      sym_global_constant_decl,
      sym_static_assert_statement,
  [4969] = 17,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(251), 1,
      sym_ident,
    ACTIONS(275), 1,
      sym_var,
    ACTIONS(285), 1,
      sym_paren_left,
    ACTIONS(289), 1,
      sym_underscore,
    ACTIONS(526), 1,
      sym_semicolon,
    STATE(126), 1,
      sym__comment,
    STATE(159), 1,
      sym_core_lhs_expression,
    STATE(166), 1,
      sym_lhs_expression,
    STATE(222), 1,
      aux_sym_lhs_expression_repeat1,
    STATE(299), 1,
      sym_variable_decl,
    STATE(364), 1,
      sym_for_init,
    STATE(365), 1,
      sym_for_header,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(257), 2,
      sym_const,
      sym_let,
    ACTIONS(279), 2,
      sym_and,
      sym_star,
    STATE(366), 5,
      sym_variable_statement,
      sym_assignment_statement,
      sym_increment_statement,
      sym_decrement_statement,
      sym_func_call_statement,
  [5028] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(127), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(489), 3,
      sym_forward_slash,
      sym_greater_than,
      sym_less_than,
    ACTIONS(487), 16,
      sym_and_and,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
      sym_modulo,
      sym_minus,
      sym_plus,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
  [5062] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(128), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(530), 3,
      sym_forward_slash,
      sym_greater_than,
      sym_less_than,
    ACTIONS(528), 16,
      sym_and_and,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
      sym_modulo,
      sym_minus,
      sym_plus,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
  [5096] = 7,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(534), 1,
      sym_forward_slash,
    STATE(129), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(536), 2,
      sym_greater_than,
      sym_less_than,
    ACTIONS(538), 2,
      sym_modulo,
      sym_star,
    ACTIONS(532), 14,
      sym_and_and,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
      sym_minus,
      sym_plus,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
  [5134] = 7,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(534), 1,
      sym_forward_slash,
    STATE(130), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(538), 2,
      sym_modulo,
      sym_star,
    ACTIONS(542), 2,
      sym_greater_than,
      sym_less_than,
    ACTIONS(540), 14,
      sym_and_and,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
      sym_minus,
      sym_plus,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
  [5172] = 13,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(35), 1,
      aux_sym_decimal_float_literal_token2,
    ACTIONS(39), 1,
      aux_sym_hex_float_literal_token2,
    ACTIONS(544), 1,
      sym_ident,
    STATE(131), 1,
      sym__comment,
    STATE(133), 1,
      aux_sym_attribute_repeat1,
    STATE(280), 1,
      sym_literal_or_ident,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(37), 2,
      aux_sym_hex_float_literal_token1,
      aux_sym_hex_float_literal_token3,
    STATE(87), 2,
      sym_decimal_float_literal,
      sym_hex_float_literal,
    STATE(276), 2,
      sym_int_literal,
      sym_float_literal,
    ACTIONS(31), 3,
      aux_sym_int_literal_token1,
      aux_sym_int_literal_token2,
      aux_sym_int_literal_token3,
    ACTIONS(33), 4,
      aux_sym_decimal_float_literal_token1,
      aux_sym_decimal_float_literal_token3,
      aux_sym_decimal_float_literal_token4,
      aux_sym_decimal_float_literal_token5,
  [5221] = 13,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(35), 1,
      aux_sym_decimal_float_literal_token2,
    ACTIONS(39), 1,
      aux_sym_hex_float_literal_token2,
    ACTIONS(544), 1,
      sym_ident,
    STATE(131), 1,
      aux_sym_attribute_repeat1,
    STATE(132), 1,
      sym__comment,
    STATE(275), 1,
      sym_literal_or_ident,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(37), 2,
      aux_sym_hex_float_literal_token1,
      aux_sym_hex_float_literal_token3,
    STATE(87), 2,
      sym_decimal_float_literal,
      sym_hex_float_literal,
    STATE(276), 2,
      sym_int_literal,
      sym_float_literal,
    ACTIONS(31), 3,
      aux_sym_int_literal_token1,
      aux_sym_int_literal_token2,
      aux_sym_int_literal_token3,
    ACTIONS(33), 4,
      aux_sym_decimal_float_literal_token1,
      aux_sym_decimal_float_literal_token3,
      aux_sym_decimal_float_literal_token4,
      aux_sym_decimal_float_literal_token5,
  [5270] = 12,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(546), 1,
      sym_ident,
    ACTIONS(555), 1,
      aux_sym_decimal_float_literal_token2,
    ACTIONS(561), 1,
      aux_sym_hex_float_literal_token2,
    STATE(342), 1,
      sym_literal_or_ident,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(558), 2,
      aux_sym_hex_float_literal_token1,
      aux_sym_hex_float_literal_token3,
    STATE(87), 2,
      sym_decimal_float_literal,
      sym_hex_float_literal,
    STATE(133), 2,
      sym__comment,
      aux_sym_attribute_repeat1,
    STATE(276), 2,
      sym_int_literal,
      sym_float_literal,
    ACTIONS(549), 3,
      aux_sym_int_literal_token1,
      aux_sym_int_literal_token2,
      aux_sym_int_literal_token3,
    ACTIONS(552), 4,
      aux_sym_decimal_float_literal_token1,
      aux_sym_decimal_float_literal_token3,
      aux_sym_decimal_float_literal_token4,
      aux_sym_decimal_float_literal_token5,
  [5317] = 6,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(357), 1,
      sym_forward_slash,
    ACTIONS(361), 1,
      sym_paren_left,
    STATE(134), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(359), 16,
      sym_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_greater_than,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [5352] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(385), 1,
      sym_forward_slash,
    STATE(135), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(387), 16,
      sym_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_greater_than,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [5384] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(417), 1,
      sym_forward_slash,
    STATE(136), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(419), 16,
      sym_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_greater_than,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [5416] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(409), 1,
      sym_forward_slash,
    STATE(137), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(411), 16,
      sym_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_greater_than,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [5448] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(138), 1,
      sym__comment,
    STATE(391), 1,
      sym_texel_format,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(564), 16,
      anon_sym_rgba8unorm,
      anon_sym_rgba8snorm,
      anon_sym_rgba8uint,
      anon_sym_rgba8sint,
      anon_sym_rgba16uint,
      anon_sym_rgba16sint,
      anon_sym_rgba16float,
      anon_sym_r32uint,
      anon_sym_r32sint,
      anon_sym_r32float,
      anon_sym_rg32uint,
      anon_sym_rg32sint,
      anon_sym_rg32float,
      anon_sym_rgba32uint,
      anon_sym_rgba32sint,
      anon_sym_rgba32float,
  [5480] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(381), 1,
      sym_forward_slash,
    STATE(139), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(383), 16,
      sym_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_greater_than,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [5512] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(401), 1,
      sym_forward_slash,
    STATE(140), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(403), 16,
      sym_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_greater_than,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [5544] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(377), 1,
      sym_forward_slash,
    STATE(141), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(379), 16,
      sym_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_greater_than,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [5576] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(389), 1,
      sym_forward_slash,
    STATE(142), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(391), 16,
      sym_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_greater_than,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [5608] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(397), 1,
      sym_forward_slash,
    STATE(143), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(399), 16,
      sym_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_greater_than,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [5640] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(405), 1,
      sym_forward_slash,
    STATE(144), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(407), 16,
      sym_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_greater_than,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [5672] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(393), 1,
      sym_forward_slash,
    STATE(145), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(395), 16,
      sym_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_greater_than,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [5704] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(357), 1,
      sym_forward_slash,
    STATE(146), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(359), 16,
      sym_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_greater_than,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [5736] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(363), 1,
      sym_forward_slash,
    STATE(147), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(365), 16,
      sym_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_greater_than,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [5768] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(421), 1,
      sym_forward_slash,
    STATE(148), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(423), 16,
      sym_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_greater_than,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [5800] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(413), 1,
      sym_forward_slash,
    STATE(149), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(415), 16,
      sym_and,
      sym_bracket_left,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_greater_than,
      sym_modulo,
      sym_minus,
      sym_period,
      sym_plus,
      sym_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [5832] = 7,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(150), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(570), 2,
      sym_greater_than,
      sym_less_than,
    ACTIONS(572), 2,
      sym_minus,
      sym_plus,
    ACTIONS(568), 4,
      sym_equal_equal,
      sym_not_equal,
      sym_greater_than_equal,
      sym_less_than_equal,
    ACTIONS(566), 8,
      sym_and_and,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
  [5867] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(477), 1,
      sym_forward_slash,
    STATE(151), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(479), 14,
      sym_and,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_greater_than,
      sym_modulo,
      sym_minus,
      sym_plus,
      sym_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [5897] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(481), 1,
      sym_forward_slash,
    STATE(152), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(483), 14,
      sym_and,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_greater_than,
      sym_modulo,
      sym_minus,
      sym_plus,
      sym_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [5927] = 6,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(576), 1,
      sym_paren_left,
    STATE(153), 1,
      sym__comment,
    STATE(269), 1,
      sym_argument_expression_list,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(574), 13,
      sym_bracket_left,
      sym_equal,
      sym_minus_minus,
      sym_period,
      sym_plus_plus,
      sym_plus_equal,
      sym_minus_equal,
      sym_times_equal,
      sym_division_equal,
      sym_modulo_equal,
      sym_and_equal,
      sym_or_equal,
      sym_xor_equal,
  [5959] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(473), 1,
      sym_forward_slash,
    STATE(154), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(475), 14,
      sym_and,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_greater_than,
      sym_modulo,
      sym_minus,
      sym_plus,
      sym_or,
      sym_paren_right,
      sym_semicolon,
      sym_star,
      sym_xor,
  [5989] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(155), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(574), 14,
      sym_bracket_left,
      sym_equal,
      sym_minus_minus,
      sym_period,
      sym_plus_plus,
      sym_paren_right,
      sym_plus_equal,
      sym_minus_equal,
      sym_times_equal,
      sym_division_equal,
      sym_modulo_equal,
      sym_and_equal,
      sym_or_equal,
      sym_xor_equal,
  [6016] = 13,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(285), 1,
      sym_paren_left,
    ACTIONS(289), 1,
      sym_underscore,
    ACTIONS(578), 1,
      sym_ident,
    ACTIONS(580), 1,
      sym_paren_right,
    STATE(156), 1,
      sym__comment,
    STATE(159), 1,
      sym_core_lhs_expression,
    STATE(166), 1,
      sym_lhs_expression,
    STATE(222), 1,
      aux_sym_lhs_expression_repeat1,
    STATE(346), 1,
      sym_for_update,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(279), 2,
      sym_and,
      sym_star,
    STATE(322), 4,
      sym_assignment_statement,
      sym_increment_statement,
      sym_decrement_statement,
      sym_func_call_statement,
  [6061] = 13,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(285), 1,
      sym_paren_left,
    ACTIONS(289), 1,
      sym_underscore,
    ACTIONS(578), 1,
      sym_ident,
    ACTIONS(582), 1,
      sym_paren_right,
    STATE(157), 1,
      sym__comment,
    STATE(159), 1,
      sym_core_lhs_expression,
    STATE(166), 1,
      sym_lhs_expression,
    STATE(222), 1,
      aux_sym_lhs_expression_repeat1,
    STATE(332), 1,
      sym_for_update,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(279), 2,
      sym_and,
      sym_star,
    STATE(322), 4,
      sym_assignment_statement,
      sym_increment_statement,
      sym_decrement_statement,
      sym_func_call_statement,
  [6106] = 7,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(586), 1,
      sym_enable,
    STATE(173), 1,
      sym_global_directive,
    STATE(175), 1,
      sym_enable_directive,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    STATE(158), 2,
      sym__comment,
      aux_sym_translation_unit_repeat1,
    ACTIONS(584), 10,
      ts_builtin_sym_end,
      sym_override,
      sym_staticAssert,
      sym_struct,
      sym_const,
      sym_fn,
      sym_type,
      sym_var,
      sym_attr,
      sym_semicolon,
  [6139] = 7,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(367), 1,
      sym_bracket_left,
    ACTIONS(369), 1,
      sym_period,
    STATE(159), 1,
      sym__comment,
    STATE(169), 1,
      sym_postfix_expression,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(589), 11,
      sym_equal,
      sym_minus_minus,
      sym_plus_plus,
      sym_plus_equal,
      sym_minus_equal,
      sym_times_equal,
      sym_division_equal,
      sym_modulo_equal,
      sym_and_equal,
      sym_or_equal,
      sym_xor_equal,
  [6172] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(160), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(591), 14,
      sym_bracket_left,
      sym_equal,
      sym_minus_minus,
      sym_period,
      sym_plus_plus,
      sym_paren_right,
      sym_plus_equal,
      sym_minus_equal,
      sym_times_equal,
      sym_division_equal,
      sym_modulo_equal,
      sym_and_equal,
      sym_or_equal,
      sym_xor_equal,
  [6199] = 13,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(285), 1,
      sym_paren_left,
    ACTIONS(289), 1,
      sym_underscore,
    ACTIONS(578), 1,
      sym_ident,
    ACTIONS(593), 1,
      sym_paren_right,
    STATE(159), 1,
      sym_core_lhs_expression,
    STATE(161), 1,
      sym__comment,
    STATE(166), 1,
      sym_lhs_expression,
    STATE(222), 1,
      aux_sym_lhs_expression_repeat1,
    STATE(323), 1,
      sym_for_update,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(279), 2,
      sym_and,
      sym_star,
    STATE(322), 4,
      sym_assignment_statement,
      sym_increment_statement,
      sym_decrement_statement,
      sym_func_call_statement,
  [6244] = 7,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(367), 1,
      sym_bracket_left,
    ACTIONS(369), 1,
      sym_period,
    STATE(162), 1,
      sym__comment,
    STATE(168), 1,
      sym_postfix_expression,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(595), 11,
      sym_equal,
      sym_minus_minus,
      sym_plus_plus,
      sym_plus_equal,
      sym_minus_equal,
      sym_times_equal,
      sym_division_equal,
      sym_modulo_equal,
      sym_and_equal,
      sym_or_equal,
      sym_xor_equal,
  [6277] = 7,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(367), 1,
      sym_bracket_left,
    ACTIONS(369), 1,
      sym_period,
    STATE(151), 1,
      sym_postfix_expression,
    STATE(163), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(341), 10,
      sym_and,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_greater_than,
      sym_or,
      sym_paren_right,
      sym_semicolon,
      sym_xor,
  [6309] = 6,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(601), 1,
      sym_paren_right,
    STATE(164), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(599), 5,
      aux_sym_int_literal_token1,
      aux_sym_int_literal_token2,
      aux_sym_int_literal_token3,
      aux_sym_decimal_float_literal_token2,
      aux_sym_hex_float_literal_token2,
    ACTIONS(597), 7,
      aux_sym_decimal_float_literal_token1,
      aux_sym_decimal_float_literal_token3,
      aux_sym_decimal_float_literal_token4,
      aux_sym_decimal_float_literal_token5,
      aux_sym_hex_float_literal_token1,
      aux_sym_hex_float_literal_token3,
      sym_ident,
  [6339] = 6,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(603), 1,
      sym_paren_right,
    STATE(165), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(599), 5,
      aux_sym_int_literal_token1,
      aux_sym_int_literal_token2,
      aux_sym_int_literal_token3,
      aux_sym_decimal_float_literal_token2,
      aux_sym_hex_float_literal_token2,
    ACTIONS(597), 7,
      aux_sym_decimal_float_literal_token1,
      aux_sym_decimal_float_literal_token3,
      aux_sym_decimal_float_literal_token4,
      aux_sym_decimal_float_literal_token5,
      aux_sym_hex_float_literal_token1,
      aux_sym_hex_float_literal_token3,
      sym_ident,
  [6369] = 8,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(605), 1,
      sym_equal,
    ACTIONS(607), 1,
      sym_minus_minus,
    ACTIONS(609), 1,
      sym_plus_plus,
    STATE(14), 1,
      sym_compound_assignment_operator,
    STATE(166), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(611), 8,
      sym_plus_equal,
      sym_minus_equal,
      sym_times_equal,
      sym_division_equal,
      sym_modulo_equal,
      sym_and_equal,
      sym_or_equal,
      sym_xor_equal,
  [6402] = 8,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(351), 1,
      sym_forward_slash,
    ACTIONS(613), 1,
      sym_bracket_left,
    ACTIONS(615), 1,
      sym_period,
    STATE(167), 1,
      sym__comment,
    STATE(187), 1,
      sym_postfix_expression,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(353), 8,
      sym_and,
      sym_greater_than,
      sym_modulo,
      sym_minus,
      sym_plus,
      sym_or,
      sym_star,
      sym_xor,
  [6435] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(168), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(617), 12,
      sym_equal,
      sym_minus_minus,
      sym_plus_plus,
      sym_paren_right,
      sym_plus_equal,
      sym_minus_equal,
      sym_times_equal,
      sym_division_equal,
      sym_modulo_equal,
      sym_and_equal,
      sym_or_equal,
      sym_xor_equal,
  [6460] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(169), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(595), 12,
      sym_equal,
      sym_minus_minus,
      sym_plus_plus,
      sym_paren_right,
      sym_plus_equal,
      sym_minus_equal,
      sym_times_equal,
      sym_division_equal,
      sym_modulo_equal,
      sym_and_equal,
      sym_or_equal,
      sym_xor_equal,
  [6485] = 8,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(347), 1,
      sym_forward_slash,
    ACTIONS(613), 1,
      sym_bracket_left,
    ACTIONS(615), 1,
      sym_period,
    STATE(170), 1,
      sym__comment,
    STATE(185), 1,
      sym_postfix_expression,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(349), 8,
      sym_and,
      sym_greater_than,
      sym_modulo,
      sym_minus,
      sym_plus,
      sym_or,
      sym_star,
      sym_xor,
  [6518] = 8,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(339), 1,
      sym_forward_slash,
    ACTIONS(613), 1,
      sym_bracket_left,
    ACTIONS(615), 1,
      sym_period,
    STATE(151), 1,
      sym_postfix_expression,
    STATE(171), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(341), 8,
      sym_and,
      sym_greater_than,
      sym_modulo,
      sym_minus,
      sym_plus,
      sym_or,
      sym_star,
      sym_xor,
  [6551] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(172), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(599), 5,
      aux_sym_int_literal_token1,
      aux_sym_int_literal_token2,
      aux_sym_int_literal_token3,
      aux_sym_decimal_float_literal_token2,
      aux_sym_hex_float_literal_token2,
    ACTIONS(597), 7,
      aux_sym_decimal_float_literal_token1,
      aux_sym_decimal_float_literal_token3,
      aux_sym_decimal_float_literal_token4,
      aux_sym_decimal_float_literal_token5,
      aux_sym_hex_float_literal_token1,
      aux_sym_hex_float_literal_token3,
      sym_ident,
  [6578] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(173), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(619), 11,
      ts_builtin_sym_end,
      sym_override,
      sym_staticAssert,
      sym_struct,
      sym_const,
      sym_enable,
      sym_fn,
      sym_type,
      sym_var,
      sym_attr,
      sym_semicolon,
  [6602] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(174), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(621), 11,
      ts_builtin_sym_end,
      sym_override,
      sym_staticAssert,
      sym_struct,
      sym_const,
      sym_enable,
      sym_fn,
      sym_type,
      sym_var,
      sym_attr,
      sym_semicolon,
  [6626] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(175), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(623), 11,
      ts_builtin_sym_end,
      sym_override,
      sym_staticAssert,
      sym_struct,
      sym_const,
      sym_enable,
      sym_fn,
      sym_type,
      sym_var,
      sym_attr,
      sym_semicolon,
  [6650] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(176), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(625), 10,
      ts_builtin_sym_end,
      sym_override,
      sym_staticAssert,
      sym_struct,
      sym_const,
      sym_fn,
      sym_type,
      sym_var,
      sym_attr,
      sym_semicolon,
  [6673] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(177), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(572), 2,
      sym_minus,
      sym_plus,
    ACTIONS(627), 8,
      sym_and_and,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_or_or,
      sym_paren_right,
      sym_semicolon,
  [6698] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(178), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(629), 10,
      ts_builtin_sym_end,
      sym_override,
      sym_staticAssert,
      sym_struct,
      sym_const,
      sym_fn,
      sym_type,
      sym_var,
      sym_attr,
      sym_semicolon,
  [6721] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(179), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(631), 10,
      ts_builtin_sym_end,
      sym_override,
      sym_staticAssert,
      sym_struct,
      sym_const,
      sym_fn,
      sym_type,
      sym_var,
      sym_attr,
      sym_semicolon,
  [6744] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(180), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(495), 10,
      ts_builtin_sym_end,
      sym_override,
      sym_staticAssert,
      sym_struct,
      sym_const,
      sym_fn,
      sym_type,
      sym_var,
      sym_attr,
      sym_semicolon,
  [6767] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(181), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(633), 10,
      ts_builtin_sym_end,
      sym_override,
      sym_staticAssert,
      sym_struct,
      sym_const,
      sym_fn,
      sym_type,
      sym_var,
      sym_attr,
      sym_semicolon,
  [6790] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(182), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(635), 10,
      ts_builtin_sym_end,
      sym_override,
      sym_staticAssert,
      sym_struct,
      sym_const,
      sym_fn,
      sym_type,
      sym_var,
      sym_attr,
      sym_semicolon,
  [6813] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(183), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(637), 10,
      ts_builtin_sym_end,
      sym_override,
      sym_staticAssert,
      sym_struct,
      sym_const,
      sym_fn,
      sym_type,
      sym_var,
      sym_attr,
      sym_semicolon,
  [6836] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(184), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(639), 10,
      ts_builtin_sym_end,
      sym_override,
      sym_staticAssert,
      sym_struct,
      sym_const,
      sym_fn,
      sym_type,
      sym_var,
      sym_attr,
      sym_semicolon,
  [6859] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(469), 1,
      sym_forward_slash,
    STATE(185), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(471), 8,
      sym_and,
      sym_greater_than,
      sym_modulo,
      sym_minus,
      sym_plus,
      sym_or,
      sym_star,
      sym_xor,
  [6883] = 8,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(489), 1,
      sym_forward_slash,
    ACTIONS(493), 1,
      sym_xor,
    ACTIONS(641), 1,
      sym_and,
    ACTIONS(643), 1,
      sym_or,
    STATE(186), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(487), 5,
      sym_greater_than,
      sym_modulo,
      sym_minus,
      sym_plus,
      sym_star,
  [6913] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(347), 1,
      sym_forward_slash,
    STATE(187), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(349), 8,
      sym_and,
      sym_greater_than,
      sym_modulo,
      sym_minus,
      sym_plus,
      sym_or,
      sym_star,
      sym_xor,
  [6937] = 12,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(25), 1,
      sym_attr,
    ACTIONS(645), 1,
      sym_ident,
    ACTIONS(647), 1,
      sym_paren_right,
    STATE(75), 1,
      sym_attribute,
    STATE(188), 1,
      sym__comment,
    STATE(205), 1,
      aux_sym_param_list_repeat1,
    STATE(225), 1,
      aux_sym_struct_member_repeat1,
    STATE(277), 1,
      sym_param,
    STATE(278), 1,
      sym_variable_ident_decl,
    STATE(388), 1,
      sym_param_list,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [6975] = 6,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(649), 1,
      sym_and_and,
    ACTIONS(653), 1,
      sym_or_or,
    STATE(189), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(651), 6,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_paren_right,
      sym_semicolon,
  [7000] = 11,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(19), 1,
      sym_fn,
    ACTIONS(23), 1,
      sym_var,
    ACTIONS(25), 1,
      sym_attr,
    ACTIONS(655), 1,
      sym_override,
    STATE(65), 1,
      aux_sym_struct_member_repeat1,
    STATE(75), 1,
      sym_attribute,
    STATE(190), 1,
      sym__comment,
    STATE(289), 1,
      sym_variable_decl,
    STATE(317), 1,
      sym_function_header,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [7035] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(191), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(657), 8,
      sym_brace_left,
      sym_brace_right,
      sym_comma,
      sym_equal,
      sym_greater_than,
      sym_paren_left,
      sym_paren_right,
      sym_semicolon,
  [7056] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(192), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(659), 8,
      sym_brace_left,
      sym_brace_right,
      sym_comma,
      sym_equal,
      sym_greater_than,
      sym_paren_left,
      sym_paren_right,
      sym_semicolon,
  [7077] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(193), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(661), 8,
      sym_brace_left,
      sym_brace_right,
      sym_comma,
      sym_equal,
      sym_greater_than,
      sym_paren_left,
      sym_paren_right,
      sym_semicolon,
  [7098] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(194), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(663), 8,
      sym_brace_left,
      sym_brace_right,
      sym_comma,
      sym_equal,
      sym_greater_than,
      sym_paren_left,
      sym_paren_right,
      sym_semicolon,
  [7119] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(195), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(665), 8,
      sym_brace_left,
      sym_brace_right,
      sym_comma,
      sym_equal,
      sym_greater_than,
      sym_paren_left,
      sym_paren_right,
      sym_semicolon,
  [7140] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(196), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(667), 8,
      sym_brace_left,
      sym_brace_right,
      sym_comma,
      sym_equal,
      sym_greater_than,
      sym_paren_left,
      sym_paren_right,
      sym_semicolon,
  [7161] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(671), 1,
      sym_xor,
    STATE(197), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(669), 7,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_greater_than,
      sym_paren_right,
      sym_semicolon,
  [7184] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(673), 1,
      sym_and,
    STATE(198), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(669), 7,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_greater_than,
      sym_paren_right,
      sym_semicolon,
  [7207] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(199), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(675), 8,
      sym_brace_left,
      sym_brace_right,
      sym_comma,
      sym_equal,
      sym_greater_than,
      sym_paren_left,
      sym_paren_right,
      sym_semicolon,
  [7228] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(200), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(677), 8,
      sym_brace_left,
      sym_brace_right,
      sym_comma,
      sym_equal,
      sym_greater_than,
      sym_paren_left,
      sym_paren_right,
      sym_semicolon,
  [7249] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(679), 1,
      sym_or,
    STATE(201), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(669), 7,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_greater_than,
      sym_paren_right,
      sym_semicolon,
  [7272] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(202), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(681), 8,
      sym_brace_left,
      sym_brace_right,
      sym_comma,
      sym_equal,
      sym_greater_than,
      sym_paren_left,
      sym_paren_right,
      sym_semicolon,
  [7293] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(203), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(683), 8,
      sym_brace_left,
      sym_brace_right,
      sym_comma,
      sym_equal,
      sym_greater_than,
      sym_paren_left,
      sym_paren_right,
      sym_semicolon,
  [7314] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(204), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(685), 8,
      sym_brace_left,
      sym_brace_right,
      sym_comma,
      sym_equal,
      sym_greater_than,
      sym_paren_left,
      sym_paren_right,
      sym_semicolon,
  [7335] = 10,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(25), 1,
      sym_attr,
    ACTIONS(645), 1,
      sym_ident,
    STATE(75), 1,
      sym_attribute,
    STATE(205), 1,
      sym__comment,
    STATE(211), 1,
      aux_sym_param_list_repeat1,
    STATE(225), 1,
      aux_sym_struct_member_repeat1,
    STATE(278), 1,
      sym_variable_ident_decl,
    STATE(298), 1,
      sym_param,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [7367] = 9,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(687), 1,
      sym_ident,
    ACTIONS(690), 1,
      sym_attr,
    STATE(75), 1,
      sym_attribute,
    STATE(223), 1,
      aux_sym_struct_member_repeat1,
    STATE(282), 1,
      sym_variable_ident_decl,
    STATE(353), 1,
      sym_struct_member,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    STATE(206), 2,
      sym__comment,
      aux_sym_struct_body_decl_repeat1,
  [7397] = 10,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(25), 1,
      sym_attr,
    ACTIONS(645), 1,
      sym_ident,
    STATE(75), 1,
      sym_attribute,
    STATE(207), 1,
      sym__comment,
    STATE(210), 1,
      aux_sym_struct_body_decl_repeat1,
    STATE(223), 1,
      aux_sym_struct_member_repeat1,
    STATE(282), 1,
      sym_variable_ident_decl,
    STATE(285), 1,
      sym_struct_member,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [7429] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(693), 1,
      sym_and_and,
    STATE(208), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(695), 6,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_paren_right,
      sym_semicolon,
  [7451] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(697), 1,
      sym_or_or,
    STATE(209), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(695), 6,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_paren_right,
      sym_semicolon,
  [7473] = 10,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(25), 1,
      sym_attr,
    ACTIONS(645), 1,
      sym_ident,
    STATE(75), 1,
      sym_attribute,
    STATE(206), 1,
      aux_sym_struct_body_decl_repeat1,
    STATE(210), 1,
      sym__comment,
    STATE(223), 1,
      aux_sym_struct_member_repeat1,
    STATE(261), 1,
      sym_struct_member,
    STATE(282), 1,
      sym_variable_ident_decl,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [7505] = 9,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(699), 1,
      sym_ident,
    ACTIONS(702), 1,
      sym_attr,
    STATE(75), 1,
      sym_attribute,
    STATE(225), 1,
      aux_sym_struct_member_repeat1,
    STATE(278), 1,
      sym_variable_ident_decl,
    STATE(349), 1,
      sym_param,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    STATE(211), 2,
      sym__comment,
      aux_sym_param_list_repeat1,
  [7535] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(212), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(705), 7,
      sym_brace_left,
      sym_brace_right,
      sym_comma,
      sym_equal,
      sym_greater_than,
      sym_paren_right,
      sym_semicolon,
  [7555] = 9,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(285), 1,
      sym_paren_left,
    ACTIONS(707), 1,
      sym_ident,
    STATE(213), 1,
      sym__comment,
    STATE(219), 1,
      aux_sym_lhs_expression_repeat1,
    STATE(239), 1,
      sym_core_lhs_expression,
    STATE(380), 1,
      sym_lhs_expression,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(279), 2,
      sym_and,
      sym_star,
  [7585] = 6,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(709), 1,
      sym_forward_slash,
    STATE(214), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(711), 2,
      sym_modulo,
      sym_star,
    ACTIONS(540), 3,
      sym_greater_than,
      sym_minus,
      sym_plus,
  [7608] = 6,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(709), 1,
      sym_forward_slash,
    STATE(215), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(711), 2,
      sym_modulo,
      sym_star,
    ACTIONS(532), 3,
      sym_greater_than,
      sym_minus,
      sym_plus,
  [7631] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(489), 1,
      sym_forward_slash,
    STATE(216), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(487), 5,
      sym_greater_than,
      sym_modulo,
      sym_minus,
      sym_plus,
      sym_star,
  [7652] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(217), 1,
      sym__comment,
    STATE(395), 1,
      sym_address_space,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(713), 5,
      sym_function,
      sym_private,
      sym_storage,
      sym_uniform,
      sym_workgroup,
  [7673] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(530), 1,
      sym_forward_slash,
    STATE(218), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(528), 5,
      sym_greater_than,
      sym_modulo,
      sym_minus,
      sym_plus,
      sym_star,
  [7694] = 8,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(285), 1,
      sym_paren_left,
    ACTIONS(707), 1,
      sym_ident,
    STATE(219), 1,
      sym__comment,
    STATE(228), 1,
      aux_sym_lhs_expression_repeat1,
    STATE(230), 1,
      sym_core_lhs_expression,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(279), 2,
      sym_and,
      sym_star,
  [7721] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(220), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(651), 6,
      sym_bracket_right,
      sym_brace_left,
      sym_colon,
      sym_comma,
      sym_paren_right,
      sym_semicolon,
  [7740] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(221), 1,
      sym__comment,
    STATE(310), 1,
      sym_address_space,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(713), 5,
      sym_function,
      sym_private,
      sym_storage,
      sym_uniform,
      sym_workgroup,
  [7761] = 8,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(285), 1,
      sym_paren_left,
    ACTIONS(707), 1,
      sym_ident,
    STATE(162), 1,
      sym_core_lhs_expression,
    STATE(222), 1,
      sym__comment,
    STATE(228), 1,
      aux_sym_lhs_expression_repeat1,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(279), 2,
      sym_and,
      sym_star,
  [7788] = 8,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(25), 1,
      sym_attr,
    ACTIONS(645), 1,
      sym_ident,
    STATE(65), 1,
      aux_sym_struct_member_repeat1,
    STATE(75), 1,
      sym_attribute,
    STATE(223), 1,
      sym__comment,
    STATE(267), 1,
      sym_variable_ident_decl,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [7814] = 8,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(715), 1,
      sym_case,
    ACTIONS(717), 1,
      sym_default,
    ACTIONS(719), 1,
      sym_brace_right,
    STATE(224), 1,
      sym__comment,
    STATE(227), 1,
      aux_sym_switch_statement_repeat1,
    STATE(248), 1,
      sym_switch_body,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [7840] = 8,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(25), 1,
      sym_attr,
    ACTIONS(645), 1,
      sym_ident,
    STATE(65), 1,
      aux_sym_struct_member_repeat1,
    STATE(75), 1,
      sym_attribute,
    STATE(225), 1,
      sym__comment,
    STATE(308), 1,
      sym_variable_ident_decl,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [7866] = 7,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(281), 1,
      sym_brace_left,
    ACTIONS(721), 1,
      sym_if,
    STATE(110), 1,
      sym_else_statement,
    STATE(226), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    STATE(104), 2,
      sym_compound_statement,
      sym_if_statement,
  [7890] = 7,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(723), 1,
      sym_case,
    ACTIONS(726), 1,
      sym_default,
    ACTIONS(729), 1,
      sym_brace_right,
    STATE(248), 1,
      sym_switch_body,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    STATE(227), 2,
      sym__comment,
      aux_sym_switch_statement_repeat1,
  [7914] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(731), 2,
      sym_paren_left,
      sym_ident,
    ACTIONS(733), 2,
      sym_and,
      sym_star,
    STATE(228), 2,
      sym__comment,
      aux_sym_lhs_expression_repeat1,
  [7934] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(229), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(736), 5,
      sym_brace_right,
      sym_comma,
      sym_equal,
      sym_paren_right,
      sym_semicolon,
  [7952] = 7,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(343), 1,
      sym_bracket_left,
    ACTIONS(345), 1,
      sym_period,
    ACTIONS(595), 1,
      sym_paren_right,
    STATE(168), 1,
      sym_postfix_expression,
    STATE(230), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [7975] = 6,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(738), 1,
      anon_sym_read,
    STATE(231), 1,
      sym__comment,
    STATE(367), 1,
      sym_access_mode,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(740), 2,
      anon_sym_write,
      anon_sym_read_write,
  [7996] = 6,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(738), 1,
      anon_sym_read,
    STATE(232), 1,
      sym__comment,
    STATE(330), 1,
      sym_access_mode,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(740), 2,
      anon_sym_write,
      anon_sym_read_write,
  [8017] = 7,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(742), 1,
      sym_ident,
    ACTIONS(744), 1,
      sym_less_than,
    STATE(233), 1,
      sym__comment,
    STATE(290), 1,
      sym_variable_ident_decl,
    STATE(292), 1,
      sym_variable_qualifier,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [8040] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(748), 1,
      sym_comma,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(746), 2,
      sym_brace_left,
      sym_colon,
    STATE(234), 2,
      sym__comment,
      aux_sym_case_selectors_repeat1,
  [8059] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(235), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(751), 4,
      sym_and,
      sym_paren_left,
      sym_star,
      sym_ident,
  [8076] = 6,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(738), 1,
      anon_sym_read,
    STATE(236), 1,
      sym__comment,
    STATE(359), 1,
      sym_access_mode,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(740), 2,
      anon_sym_write,
      anon_sym_read_write,
  [8097] = 7,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(715), 1,
      sym_case,
    ACTIONS(717), 1,
      sym_default,
    STATE(224), 1,
      aux_sym_switch_statement_repeat1,
    STATE(237), 1,
      sym__comment,
    STATE(248), 1,
      sym_switch_body,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [8120] = 6,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(753), 1,
      sym_comma,
    STATE(234), 1,
      aux_sym_case_selectors_repeat1,
    STATE(238), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(73), 2,
      sym_brace_left,
      sym_colon,
  [8141] = 7,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(343), 1,
      sym_bracket_left,
    ACTIONS(345), 1,
      sym_period,
    ACTIONS(589), 1,
      sym_paren_right,
    STATE(169), 1,
      sym_postfix_expression,
    STATE(239), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [8164] = 6,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(757), 1,
      sym_comma,
    STATE(238), 1,
      aux_sym_case_selectors_repeat1,
    STATE(240), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(755), 2,
      sym_brace_left,
      sym_colon,
  [8185] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(759), 1,
      sym_greater_than,
    STATE(241), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(761), 2,
      sym_minus,
      sym_plus,
  [8203] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(242), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(746), 3,
      sym_brace_left,
      sym_colon,
      sym_comma,
  [8219] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(243), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(763), 3,
      sym_case,
      sym_default,
      sym_brace_right,
  [8235] = 6,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(765), 1,
      sym_brace_left,
    ACTIONS(767), 1,
      sym_colon,
    STATE(244), 1,
      sym__comment,
    STATE(252), 1,
      sym_case_compound_statement,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [8255] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(769), 1,
      sym_colon,
    STATE(245), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(771), 2,
      sym_equal,
      sym_semicolon,
  [8273] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(775), 1,
      sym_paren_right,
    STATE(246), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(773), 2,
      sym_attr,
      sym_ident,
  [8291] = 6,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(769), 1,
      sym_colon,
    ACTIONS(777), 1,
      sym_equal,
    ACTIONS(779), 1,
      sym_semicolon,
    STATE(247), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [8311] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(248), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(781), 3,
      sym_case,
      sym_default,
      sym_brace_right,
  [8327] = 6,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(765), 1,
      sym_brace_left,
    ACTIONS(783), 1,
      sym_colon,
    STATE(243), 1,
      sym_case_compound_statement,
    STATE(249), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [8347] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(787), 1,
      sym_brace_right,
    STATE(250), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(785), 2,
      sym_attr,
      sym_ident,
  [8365] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(769), 1,
      sym_colon,
    STATE(251), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(789), 2,
      sym_equal,
      sym_semicolon,
  [8383] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(252), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(791), 3,
      sym_case,
      sym_default,
      sym_brace_right,
  [8399] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(253), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(793), 3,
      sym_case,
      sym_default,
      sym_brace_right,
  [8415] = 6,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(769), 1,
      sym_colon,
    ACTIONS(795), 1,
      sym_equal,
    ACTIONS(797), 1,
      sym_semicolon,
    STATE(254), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [8435] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(799), 1,
      sym_brace_right,
    STATE(255), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(785), 2,
      sym_attr,
      sym_ident,
  [8453] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(256), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(801), 3,
      sym_case,
      sym_default,
      sym_brace_right,
  [8469] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(257), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(803), 3,
      sym_case,
      sym_default,
      sym_brace_right,
  [8485] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(805), 1,
      sym_paren_right,
    STATE(258), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(773), 2,
      sym_attr,
      sym_ident,
  [8503] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(259), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(807), 3,
      sym_case,
      sym_default,
      sym_brace_right,
  [8519] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(809), 1,
      sym_equal,
    ACTIONS(811), 1,
      sym_semicolon,
    STATE(260), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [8536] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(787), 1,
      sym_brace_right,
    ACTIONS(813), 1,
      sym_comma,
    STATE(261), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [8553] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(769), 1,
      sym_colon,
    ACTIONS(795), 1,
      sym_equal,
    STATE(262), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [8570] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(281), 1,
      sym_brace_left,
    STATE(178), 1,
      sym_compound_statement,
    STATE(263), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [8587] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(264), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(815), 2,
      sym_paren_right,
      sym_semicolon,
  [8602] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(265), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(817), 2,
      sym_paren_right,
      sym_semicolon,
  [8617] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(266), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(819), 2,
      sym_paren_right,
      sym_semicolon,
  [8632] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(267), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(821), 2,
      sym_brace_right,
      sym_comma,
  [8647] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(823), 1,
      sym_ident,
    STATE(268), 1,
      sym__comment,
    STATE(297), 1,
      sym_variable_ident_decl,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [8664] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(269), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(825), 2,
      sym_paren_right,
      sym_semicolon,
  [8679] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(281), 1,
      sym_brace_left,
    STATE(109), 1,
      sym_compound_statement,
    STATE(270), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [8696] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(827), 1,
      sym_comma,
    ACTIONS(829), 1,
      sym_greater_than,
    STATE(271), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [8713] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(361), 1,
      sym_paren_left,
    ACTIONS(831), 1,
      sym_less_than,
    STATE(272), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [8730] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(281), 1,
      sym_brace_left,
    STATE(89), 1,
      sym_compound_statement,
    STATE(273), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [8747] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(769), 1,
      sym_colon,
    ACTIONS(833), 1,
      sym_equal,
    STATE(274), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [8764] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(835), 1,
      sym_comma,
    ACTIONS(837), 1,
      sym_paren_right,
    STATE(275), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [8781] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(276), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(839), 2,
      sym_comma,
      sym_paren_right,
  [8796] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(841), 1,
      sym_comma,
    ACTIONS(843), 1,
      sym_paren_right,
    STATE(277), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [8813] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(278), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(845), 2,
      sym_comma,
      sym_paren_right,
  [8828] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(847), 1,
      sym_arrow,
    ACTIONS(849), 1,
      sym_brace_left,
    STATE(279), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [8845] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(601), 1,
      sym_paren_right,
    ACTIONS(851), 1,
      sym_comma,
    STATE(280), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [8862] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(576), 1,
      sym_paren_left,
    STATE(103), 1,
      sym_argument_expression_list,
    STATE(281), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [8879] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(282), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(853), 2,
      sym_brace_right,
      sym_comma,
  [8894] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(174), 1,
      sym_paren_right,
    ACTIONS(855), 1,
      sym_comma,
    STATE(283), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [8911] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(765), 1,
      sym_brace_left,
    STATE(256), 1,
      sym_case_compound_statement,
    STATE(284), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [8928] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(857), 1,
      sym_brace_right,
    ACTIONS(859), 1,
      sym_comma,
    STATE(285), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [8945] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(861), 1,
      sym_comma,
    ACTIONS(863), 1,
      sym_paren_right,
    STATE(286), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [8962] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(281), 1,
      sym_brace_left,
    STATE(105), 1,
      sym_compound_statement,
    STATE(287), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [8979] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(288), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(865), 2,
      sym_less_than,
      sym_paren_left,
  [8994] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(867), 1,
      sym_equal,
    ACTIONS(869), 1,
      sym_semicolon,
    STATE(289), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9011] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(290), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(789), 2,
      sym_equal,
      sym_semicolon,
  [9026] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(795), 1,
      sym_equal,
    ACTIONS(797), 1,
      sym_semicolon,
    STATE(291), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9043] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(871), 1,
      sym_ident,
    STATE(292), 1,
      sym__comment,
    STATE(306), 1,
      sym_variable_ident_decl,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9060] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(873), 1,
      sym_brace_left,
    STATE(293), 1,
      sym__comment,
    STATE(338), 1,
      sym_continuing_compound_statement,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9077] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(71), 1,
      sym_paren_left,
    STATE(90), 1,
      sym_paren_expression,
    STATE(294), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9094] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(875), 1,
      sym_paren_left,
    STATE(148), 1,
      sym_argument_expression_list,
    STATE(295), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9111] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(877), 1,
      sym_comma,
    ACTIONS(879), 1,
      sym_greater_than,
    STATE(296), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9128] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(777), 1,
      sym_equal,
    ACTIONS(779), 1,
      sym_semicolon,
    STATE(297), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9145] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(775), 1,
      sym_paren_right,
    ACTIONS(881), 1,
      sym_comma,
    STATE(298), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9162] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(883), 1,
      sym_equal,
    ACTIONS(885), 1,
      sym_semicolon,
    STATE(299), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9179] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(887), 1,
      sym_comma,
    ACTIONS(889), 1,
      sym_paren_right,
    STATE(300), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9196] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(301), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(785), 2,
      sym_attr,
      sym_ident,
  [9211] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(166), 1,
      sym_paren_left,
    STATE(141), 1,
      sym_paren_expression,
    STATE(302), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9228] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(303), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(773), 2,
      sym_attr,
      sym_ident,
  [9243] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(180), 1,
      sym_paren_right,
    ACTIONS(891), 1,
      sym_comma,
    STATE(304), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9260] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(893), 1,
      sym_ident,
    STATE(305), 1,
      sym__comment,
    STATE(385), 1,
      sym_variable_ident_decl,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9277] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(306), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(771), 2,
      sym_equal,
      sym_semicolon,
  [9292] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(307), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(895), 2,
      sym_less_than,
      sym_paren_left,
  [9307] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(308), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(897), 2,
      sym_comma,
      sym_paren_right,
  [9322] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(899), 1,
      sym_if,
    ACTIONS(901), 1,
      sym_semicolon,
    STATE(309), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9339] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(903), 1,
      sym_comma,
    ACTIONS(905), 1,
      sym_greater_than,
    STATE(310), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9356] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    STATE(311), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
    ACTIONS(907), 2,
      sym_comma,
      sym_greater_than,
  [9371] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(909), 1,
      sym_ident,
    STATE(312), 1,
      sym__comment,
    STATE(376), 1,
      sym_variable_ident_decl,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9388] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(911), 1,
      sym_arrow,
    ACTIONS(913), 1,
      sym_brace_left,
    STATE(313), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9405] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(765), 1,
      sym_brace_left,
    STATE(252), 1,
      sym_case_compound_statement,
    STATE(314), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9422] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(915), 1,
      sym_brace_left,
    STATE(179), 1,
      sym_struct_body_decl,
    STATE(315), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9439] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(917), 1,
      sym_ident,
    STATE(291), 1,
      sym_variable_ident_decl,
    STATE(316), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9456] = 5,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(281), 1,
      sym_brace_left,
    STATE(181), 1,
      sym_compound_statement,
    STATE(317), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9473] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(919), 1,
      sym_greater_than,
    STATE(318), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9487] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(921), 1,
      sym_brace_right,
    STATE(319), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9501] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(311), 1,
      sym_brace_right,
    STATE(320), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9515] = 5,
    ACTIONS(5), 1,
      sym__block_comment,
    ACTIONS(923), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(925), 1,
      aux_sym__comment_token1,
    ACTIONS(927), 1,
      sym__blankspace,
    STATE(321), 1,
      sym__comment,
  [9531] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(929), 1,
      sym_paren_right,
    STATE(322), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9545] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(582), 1,
      sym_paren_right,
    STATE(323), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9559] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(931), 1,
      sym_brace_right,
    STATE(324), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9573] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(831), 1,
      sym_less_than,
    STATE(325), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9587] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(933), 1,
      sym_brace_left,
    STATE(326), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9601] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(935), 1,
      sym_semicolon,
    STATE(327), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9615] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(937), 1,
      sym_less_than,
    STATE(328), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9629] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(939), 1,
      sym_less_than,
    STATE(329), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9643] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(941), 1,
      sym_greater_than,
    STATE(330), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9657] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(943), 1,
      sym_brace_left,
    STATE(331), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9671] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(580), 1,
      sym_paren_right,
    STATE(332), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9685] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(945), 1,
      sym_less_than,
    STATE(333), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9699] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(947), 1,
      sym_less_than,
    STATE(334), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9713] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(949), 1,
      sym_brace_right,
    STATE(335), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9727] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(951), 1,
      sym_brace_right,
    STATE(336), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9741] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(953), 1,
      sym_greater_than,
    STATE(337), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9755] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(955), 1,
      sym_brace_right,
    STATE(338), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9769] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(144), 1,
      sym_semicolon,
    STATE(339), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9783] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(957), 1,
      sym_semicolon,
    STATE(340), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9797] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(959), 1,
      sym_semicolon,
    STATE(341), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9811] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(961), 1,
      sym_comma,
    STATE(342), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9825] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(303), 1,
      sym_brace_right,
    STATE(343), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9839] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(963), 1,
      sym_less_than,
    STATE(344), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9853] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(965), 1,
      sym_ident,
    STATE(345), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9867] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(967), 1,
      sym_paren_right,
    STATE(346), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9881] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(969), 1,
      sym_semicolon,
    STATE(347), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9895] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(971), 1,
      sym_brace_right,
    STATE(348), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9909] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(973), 1,
      sym_comma,
    STATE(349), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9923] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(975), 1,
      sym_less_than,
    STATE(350), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9937] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(977), 1,
      sym_brace_left,
    STATE(351), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9951] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(979), 1,
      sym_brace_right,
    STATE(352), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9965] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(981), 1,
      sym_comma,
    STATE(353), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9979] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(983), 1,
      sym_brace_right,
    STATE(354), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [9993] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(985), 1,
      sym_brace_right,
    STATE(355), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10007] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(987), 1,
      sym_comma,
    STATE(356), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10021] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(989), 1,
      sym_less_than,
    STATE(357), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10035] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(361), 1,
      sym_paren_left,
    STATE(358), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10049] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(991), 1,
      sym_greater_than,
    STATE(359), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10063] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(759), 1,
      sym_greater_than,
    STATE(360), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10077] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(993), 1,
      sym_semicolon,
    STATE(361), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10091] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(995), 1,
      sym_semicolon,
    STATE(362), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10105] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(283), 1,
      sym_brace_right,
    STATE(363), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10119] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(997), 1,
      sym_semicolon,
    STATE(364), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10133] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(999), 1,
      sym_paren_right,
    STATE(365), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10147] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1001), 1,
      sym_semicolon,
    STATE(366), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10161] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1003), 1,
      sym_greater_than,
    STATE(367), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10175] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1005), 1,
      sym_paren_right,
    STATE(368), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10189] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1007), 1,
      sym_and_and,
    STATE(369), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10203] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1009), 1,
      sym_or_or,
    STATE(370), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10217] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1011), 1,
      sym_or,
    STATE(371), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10231] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1013), 1,
      sym_and,
    STATE(372), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10245] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1015), 1,
      sym_xor,
    STATE(373), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10259] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1017), 1,
      sym_semicolon,
    STATE(374), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10273] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1019), 1,
      sym_semicolon,
    STATE(375), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10287] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(795), 1,
      sym_equal,
    STATE(376), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10301] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1021), 1,
      sym_semicolon,
    STATE(377), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10315] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1023), 1,
      sym_semicolon,
    STATE(378), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10329] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1025), 1,
      sym_paren_left,
    STATE(379), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10343] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1027), 1,
      sym_paren_right,
    STATE(380), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10357] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1029), 1,
      sym_brace_left,
    STATE(381), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10371] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1031), 1,
      sym_semicolon,
    STATE(382), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10385] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1033), 1,
      sym_less_than,
    STATE(383), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10399] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1035), 1,
      ts_builtin_sym_end,
    STATE(384), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10413] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(833), 1,
      sym_equal,
    STATE(385), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10427] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1037), 1,
      sym_ident,
    STATE(386), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10441] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1039), 1,
      sym_semicolon,
    STATE(387), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10455] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1041), 1,
      sym_paren_right,
    STATE(388), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10469] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(769), 1,
      sym_colon,
    STATE(389), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10483] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1043), 1,
      sym_bracket_right,
    STATE(390), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10497] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1045), 1,
      sym_comma,
    STATE(391), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10511] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1047), 1,
      sym_comma,
    STATE(392), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10525] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1049), 1,
      sym_greater_than,
    STATE(393), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10539] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1051), 1,
      sym_greater_than,
    STATE(394), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10553] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1053), 1,
      sym_comma,
    STATE(395), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10567] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1055), 1,
      sym_or,
    STATE(396), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10581] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1057), 1,
      sym_xor,
    STATE(397), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10595] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1059), 1,
      sym_paren_right,
    STATE(398), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10609] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1061), 1,
      sym_ident,
    STATE(399), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10623] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1063), 1,
      sym_greater_than,
    STATE(400), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10637] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1065), 1,
      sym_semicolon,
    STATE(401), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10651] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1067), 1,
      sym_bracket_right,
    STATE(402), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10665] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(605), 1,
      sym_equal,
    STATE(403), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10679] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1069), 1,
      sym_equal,
    STATE(404), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10693] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1071), 1,
      sym_brace_left,
    STATE(405), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10707] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1073), 1,
      sym_ident,
    STATE(406), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10721] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1075), 1,
      sym_paren_left,
    STATE(407), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10735] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1077), 1,
      sym_semicolon,
    STATE(408), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10749] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1079), 1,
      sym_ident,
    STATE(409), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10763] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1081), 1,
      sym_bracket_right,
    STATE(410), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10777] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1083), 1,
      sym_ident,
    STATE(411), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10791] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1085), 1,
      sym_ident,
    STATE(412), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10805] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1087), 1,
      sym_semicolon,
    STATE(413), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10819] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1089), 1,
      sym_greater_than,
    STATE(414), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10833] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1091), 1,
      sym_ident,
    STATE(415), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10847] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(901), 1,
      sym_semicolon,
    STATE(416), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10861] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1093), 1,
      sym_semicolon,
    STATE(417), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10875] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1095), 1,
      sym_ident,
    STATE(418), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10889] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1097), 1,
      sym_ident,
    STATE(419), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10903] = 4,
    ACTIONS(3), 1,
      anon_sym_SLASH_SLASH,
    ACTIONS(1099), 1,
      sym_less_than,
    STATE(420), 1,
      sym__comment,
    ACTIONS(5), 2,
      sym__block_comment,
      sym__blankspace,
  [10917] = 1,
    ACTIONS(1101), 1,
      ts_builtin_sym_end,
};

static const uint32_t ts_small_parse_table_map[] = {
  [SMALL_STATE(47)] = 0,
  [SMALL_STATE(48)] = 77,
  [SMALL_STATE(49)] = 154,
  [SMALL_STATE(50)] = 231,
  [SMALL_STATE(51)] = 308,
  [SMALL_STATE(52)] = 382,
  [SMALL_STATE(53)] = 456,
  [SMALL_STATE(54)] = 564,
  [SMALL_STATE(55)] = 672,
  [SMALL_STATE(56)] = 780,
  [SMALL_STATE(57)] = 888,
  [SMALL_STATE(58)] = 987,
  [SMALL_STATE(59)] = 1086,
  [SMALL_STATE(60)] = 1185,
  [SMALL_STATE(61)] = 1284,
  [SMALL_STATE(62)] = 1383,
  [SMALL_STATE(63)] = 1482,
  [SMALL_STATE(64)] = 1581,
  [SMALL_STATE(65)] = 1680,
  [SMALL_STATE(66)] = 1742,
  [SMALL_STATE(67)] = 1851,
  [SMALL_STATE(68)] = 1964,
  [SMALL_STATE(69)] = 2077,
  [SMALL_STATE(70)] = 2190,
  [SMALL_STATE(71)] = 2251,
  [SMALL_STATE(72)] = 2364,
  [SMALL_STATE(73)] = 2422,
  [SMALL_STATE(74)] = 2532,
  [SMALL_STATE(75)] = 2642,
  [SMALL_STATE(76)] = 2700,
  [SMALL_STATE(77)] = 2758,
  [SMALL_STATE(78)] = 2816,
  [SMALL_STATE(79)] = 2923,
  [SMALL_STATE(80)] = 3030,
  [SMALL_STATE(81)] = 3075,
  [SMALL_STATE(82)] = 3120,
  [SMALL_STATE(83)] = 3166,
  [SMALL_STATE(84)] = 3212,
  [SMALL_STATE(85)] = 3258,
  [SMALL_STATE(86)] = 3336,
  [SMALL_STATE(87)] = 3378,
  [SMALL_STATE(88)] = 3417,
  [SMALL_STATE(89)] = 3462,
  [SMALL_STATE(90)] = 3503,
  [SMALL_STATE(91)] = 3542,
  [SMALL_STATE(92)] = 3581,
  [SMALL_STATE(93)] = 3620,
  [SMALL_STATE(94)] = 3659,
  [SMALL_STATE(95)] = 3698,
  [SMALL_STATE(96)] = 3737,
  [SMALL_STATE(97)] = 3776,
  [SMALL_STATE(98)] = 3815,
  [SMALL_STATE(99)] = 3854,
  [SMALL_STATE(100)] = 3893,
  [SMALL_STATE(101)] = 3932,
  [SMALL_STATE(102)] = 3971,
  [SMALL_STATE(103)] = 4016,
  [SMALL_STATE(104)] = 4055,
  [SMALL_STATE(105)] = 4093,
  [SMALL_STATE(106)] = 4131,
  [SMALL_STATE(107)] = 4169,
  [SMALL_STATE(108)] = 4207,
  [SMALL_STATE(109)] = 4245,
  [SMALL_STATE(110)] = 4283,
  [SMALL_STATE(111)] = 4321,
  [SMALL_STATE(112)] = 4359,
  [SMALL_STATE(113)] = 4397,
  [SMALL_STATE(114)] = 4435,
  [SMALL_STATE(115)] = 4473,
  [SMALL_STATE(116)] = 4510,
  [SMALL_STATE(117)] = 4547,
  [SMALL_STATE(118)] = 4584,
  [SMALL_STATE(119)] = 4621,
  [SMALL_STATE(120)] = 4664,
  [SMALL_STATE(121)] = 4701,
  [SMALL_STATE(122)] = 4737,
  [SMALL_STATE(123)] = 4773,
  [SMALL_STATE(124)] = 4837,
  [SMALL_STATE(125)] = 4903,
  [SMALL_STATE(126)] = 4969,
  [SMALL_STATE(127)] = 5028,
  [SMALL_STATE(128)] = 5062,
  [SMALL_STATE(129)] = 5096,
  [SMALL_STATE(130)] = 5134,
  [SMALL_STATE(131)] = 5172,
  [SMALL_STATE(132)] = 5221,
  [SMALL_STATE(133)] = 5270,
  [SMALL_STATE(134)] = 5317,
  [SMALL_STATE(135)] = 5352,
  [SMALL_STATE(136)] = 5384,
  [SMALL_STATE(137)] = 5416,
  [SMALL_STATE(138)] = 5448,
  [SMALL_STATE(139)] = 5480,
  [SMALL_STATE(140)] = 5512,
  [SMALL_STATE(141)] = 5544,
  [SMALL_STATE(142)] = 5576,
  [SMALL_STATE(143)] = 5608,
  [SMALL_STATE(144)] = 5640,
  [SMALL_STATE(145)] = 5672,
  [SMALL_STATE(146)] = 5704,
  [SMALL_STATE(147)] = 5736,
  [SMALL_STATE(148)] = 5768,
  [SMALL_STATE(149)] = 5800,
  [SMALL_STATE(150)] = 5832,
  [SMALL_STATE(151)] = 5867,
  [SMALL_STATE(152)] = 5897,
  [SMALL_STATE(153)] = 5927,
  [SMALL_STATE(154)] = 5959,
  [SMALL_STATE(155)] = 5989,
  [SMALL_STATE(156)] = 6016,
  [SMALL_STATE(157)] = 6061,
  [SMALL_STATE(158)] = 6106,
  [SMALL_STATE(159)] = 6139,
  [SMALL_STATE(160)] = 6172,
  [SMALL_STATE(161)] = 6199,
  [SMALL_STATE(162)] = 6244,
  [SMALL_STATE(163)] = 6277,
  [SMALL_STATE(164)] = 6309,
  [SMALL_STATE(165)] = 6339,
  [SMALL_STATE(166)] = 6369,
  [SMALL_STATE(167)] = 6402,
  [SMALL_STATE(168)] = 6435,
  [SMALL_STATE(169)] = 6460,
  [SMALL_STATE(170)] = 6485,
  [SMALL_STATE(171)] = 6518,
  [SMALL_STATE(172)] = 6551,
  [SMALL_STATE(173)] = 6578,
  [SMALL_STATE(174)] = 6602,
  [SMALL_STATE(175)] = 6626,
  [SMALL_STATE(176)] = 6650,
  [SMALL_STATE(177)] = 6673,
  [SMALL_STATE(178)] = 6698,
  [SMALL_STATE(179)] = 6721,
  [SMALL_STATE(180)] = 6744,
  [SMALL_STATE(181)] = 6767,
  [SMALL_STATE(182)] = 6790,
  [SMALL_STATE(183)] = 6813,
  [SMALL_STATE(184)] = 6836,
  [SMALL_STATE(185)] = 6859,
  [SMALL_STATE(186)] = 6883,
  [SMALL_STATE(187)] = 6913,
  [SMALL_STATE(188)] = 6937,
  [SMALL_STATE(189)] = 6975,
  [SMALL_STATE(190)] = 7000,
  [SMALL_STATE(191)] = 7035,
  [SMALL_STATE(192)] = 7056,
  [SMALL_STATE(193)] = 7077,
  [SMALL_STATE(194)] = 7098,
  [SMALL_STATE(195)] = 7119,
  [SMALL_STATE(196)] = 7140,
  [SMALL_STATE(197)] = 7161,
  [SMALL_STATE(198)] = 7184,
  [SMALL_STATE(199)] = 7207,
  [SMALL_STATE(200)] = 7228,
  [SMALL_STATE(201)] = 7249,
  [SMALL_STATE(202)] = 7272,
  [SMALL_STATE(203)] = 7293,
  [SMALL_STATE(204)] = 7314,
  [SMALL_STATE(205)] = 7335,
  [SMALL_STATE(206)] = 7367,
  [SMALL_STATE(207)] = 7397,
  [SMALL_STATE(208)] = 7429,
  [SMALL_STATE(209)] = 7451,
  [SMALL_STATE(210)] = 7473,
  [SMALL_STATE(211)] = 7505,
  [SMALL_STATE(212)] = 7535,
  [SMALL_STATE(213)] = 7555,
  [SMALL_STATE(214)] = 7585,
  [SMALL_STATE(215)] = 7608,
  [SMALL_STATE(216)] = 7631,
  [SMALL_STATE(217)] = 7652,
  [SMALL_STATE(218)] = 7673,
  [SMALL_STATE(219)] = 7694,
  [SMALL_STATE(220)] = 7721,
  [SMALL_STATE(221)] = 7740,
  [SMALL_STATE(222)] = 7761,
  [SMALL_STATE(223)] = 7788,
  [SMALL_STATE(224)] = 7814,
  [SMALL_STATE(225)] = 7840,
  [SMALL_STATE(226)] = 7866,
  [SMALL_STATE(227)] = 7890,
  [SMALL_STATE(228)] = 7914,
  [SMALL_STATE(229)] = 7934,
  [SMALL_STATE(230)] = 7952,
  [SMALL_STATE(231)] = 7975,
  [SMALL_STATE(232)] = 7996,
  [SMALL_STATE(233)] = 8017,
  [SMALL_STATE(234)] = 8040,
  [SMALL_STATE(235)] = 8059,
  [SMALL_STATE(236)] = 8076,
  [SMALL_STATE(237)] = 8097,
  [SMALL_STATE(238)] = 8120,
  [SMALL_STATE(239)] = 8141,
  [SMALL_STATE(240)] = 8164,
  [SMALL_STATE(241)] = 8185,
  [SMALL_STATE(242)] = 8203,
  [SMALL_STATE(243)] = 8219,
  [SMALL_STATE(244)] = 8235,
  [SMALL_STATE(245)] = 8255,
  [SMALL_STATE(246)] = 8273,
  [SMALL_STATE(247)] = 8291,
  [SMALL_STATE(248)] = 8311,
  [SMALL_STATE(249)] = 8327,
  [SMALL_STATE(250)] = 8347,
  [SMALL_STATE(251)] = 8365,
  [SMALL_STATE(252)] = 8383,
  [SMALL_STATE(253)] = 8399,
  [SMALL_STATE(254)] = 8415,
  [SMALL_STATE(255)] = 8435,
  [SMALL_STATE(256)] = 8453,
  [SMALL_STATE(257)] = 8469,
  [SMALL_STATE(258)] = 8485,
  [SMALL_STATE(259)] = 8503,
  [SMALL_STATE(260)] = 8519,
  [SMALL_STATE(261)] = 8536,
  [SMALL_STATE(262)] = 8553,
  [SMALL_STATE(263)] = 8570,
  [SMALL_STATE(264)] = 8587,
  [SMALL_STATE(265)] = 8602,
  [SMALL_STATE(266)] = 8617,
  [SMALL_STATE(267)] = 8632,
  [SMALL_STATE(268)] = 8647,
  [SMALL_STATE(269)] = 8664,
  [SMALL_STATE(270)] = 8679,
  [SMALL_STATE(271)] = 8696,
  [SMALL_STATE(272)] = 8713,
  [SMALL_STATE(273)] = 8730,
  [SMALL_STATE(274)] = 8747,
  [SMALL_STATE(275)] = 8764,
  [SMALL_STATE(276)] = 8781,
  [SMALL_STATE(277)] = 8796,
  [SMALL_STATE(278)] = 8813,
  [SMALL_STATE(279)] = 8828,
  [SMALL_STATE(280)] = 8845,
  [SMALL_STATE(281)] = 8862,
  [SMALL_STATE(282)] = 8879,
  [SMALL_STATE(283)] = 8894,
  [SMALL_STATE(284)] = 8911,
  [SMALL_STATE(285)] = 8928,
  [SMALL_STATE(286)] = 8945,
  [SMALL_STATE(287)] = 8962,
  [SMALL_STATE(288)] = 8979,
  [SMALL_STATE(289)] = 8994,
  [SMALL_STATE(290)] = 9011,
  [SMALL_STATE(291)] = 9026,
  [SMALL_STATE(292)] = 9043,
  [SMALL_STATE(293)] = 9060,
  [SMALL_STATE(294)] = 9077,
  [SMALL_STATE(295)] = 9094,
  [SMALL_STATE(296)] = 9111,
  [SMALL_STATE(297)] = 9128,
  [SMALL_STATE(298)] = 9145,
  [SMALL_STATE(299)] = 9162,
  [SMALL_STATE(300)] = 9179,
  [SMALL_STATE(301)] = 9196,
  [SMALL_STATE(302)] = 9211,
  [SMALL_STATE(303)] = 9228,
  [SMALL_STATE(304)] = 9243,
  [SMALL_STATE(305)] = 9260,
  [SMALL_STATE(306)] = 9277,
  [SMALL_STATE(307)] = 9292,
  [SMALL_STATE(308)] = 9307,
  [SMALL_STATE(309)] = 9322,
  [SMALL_STATE(310)] = 9339,
  [SMALL_STATE(311)] = 9356,
  [SMALL_STATE(312)] = 9371,
  [SMALL_STATE(313)] = 9388,
  [SMALL_STATE(314)] = 9405,
  [SMALL_STATE(315)] = 9422,
  [SMALL_STATE(316)] = 9439,
  [SMALL_STATE(317)] = 9456,
  [SMALL_STATE(318)] = 9473,
  [SMALL_STATE(319)] = 9487,
  [SMALL_STATE(320)] = 9501,
  [SMALL_STATE(321)] = 9515,
  [SMALL_STATE(322)] = 9531,
  [SMALL_STATE(323)] = 9545,
  [SMALL_STATE(324)] = 9559,
  [SMALL_STATE(325)] = 9573,
  [SMALL_STATE(326)] = 9587,
  [SMALL_STATE(327)] = 9601,
  [SMALL_STATE(328)] = 9615,
  [SMALL_STATE(329)] = 9629,
  [SMALL_STATE(330)] = 9643,
  [SMALL_STATE(331)] = 9657,
  [SMALL_STATE(332)] = 9671,
  [SMALL_STATE(333)] = 9685,
  [SMALL_STATE(334)] = 9699,
  [SMALL_STATE(335)] = 9713,
  [SMALL_STATE(336)] = 9727,
  [SMALL_STATE(337)] = 9741,
  [SMALL_STATE(338)] = 9755,
  [SMALL_STATE(339)] = 9769,
  [SMALL_STATE(340)] = 9783,
  [SMALL_STATE(341)] = 9797,
  [SMALL_STATE(342)] = 9811,
  [SMALL_STATE(343)] = 9825,
  [SMALL_STATE(344)] = 9839,
  [SMALL_STATE(345)] = 9853,
  [SMALL_STATE(346)] = 9867,
  [SMALL_STATE(347)] = 9881,
  [SMALL_STATE(348)] = 9895,
  [SMALL_STATE(349)] = 9909,
  [SMALL_STATE(350)] = 9923,
  [SMALL_STATE(351)] = 9937,
  [SMALL_STATE(352)] = 9951,
  [SMALL_STATE(353)] = 9965,
  [SMALL_STATE(354)] = 9979,
  [SMALL_STATE(355)] = 9993,
  [SMALL_STATE(356)] = 10007,
  [SMALL_STATE(357)] = 10021,
  [SMALL_STATE(358)] = 10035,
  [SMALL_STATE(359)] = 10049,
  [SMALL_STATE(360)] = 10063,
  [SMALL_STATE(361)] = 10077,
  [SMALL_STATE(362)] = 10091,
  [SMALL_STATE(363)] = 10105,
  [SMALL_STATE(364)] = 10119,
  [SMALL_STATE(365)] = 10133,
  [SMALL_STATE(366)] = 10147,
  [SMALL_STATE(367)] = 10161,
  [SMALL_STATE(368)] = 10175,
  [SMALL_STATE(369)] = 10189,
  [SMALL_STATE(370)] = 10203,
  [SMALL_STATE(371)] = 10217,
  [SMALL_STATE(372)] = 10231,
  [SMALL_STATE(373)] = 10245,
  [SMALL_STATE(374)] = 10259,
  [SMALL_STATE(375)] = 10273,
  [SMALL_STATE(376)] = 10287,
  [SMALL_STATE(377)] = 10301,
  [SMALL_STATE(378)] = 10315,
  [SMALL_STATE(379)] = 10329,
  [SMALL_STATE(380)] = 10343,
  [SMALL_STATE(381)] = 10357,
  [SMALL_STATE(382)] = 10371,
  [SMALL_STATE(383)] = 10385,
  [SMALL_STATE(384)] = 10399,
  [SMALL_STATE(385)] = 10413,
  [SMALL_STATE(386)] = 10427,
  [SMALL_STATE(387)] = 10441,
  [SMALL_STATE(388)] = 10455,
  [SMALL_STATE(389)] = 10469,
  [SMALL_STATE(390)] = 10483,
  [SMALL_STATE(391)] = 10497,
  [SMALL_STATE(392)] = 10511,
  [SMALL_STATE(393)] = 10525,
  [SMALL_STATE(394)] = 10539,
  [SMALL_STATE(395)] = 10553,
  [SMALL_STATE(396)] = 10567,
  [SMALL_STATE(397)] = 10581,
  [SMALL_STATE(398)] = 10595,
  [SMALL_STATE(399)] = 10609,
  [SMALL_STATE(400)] = 10623,
  [SMALL_STATE(401)] = 10637,
  [SMALL_STATE(402)] = 10651,
  [SMALL_STATE(403)] = 10665,
  [SMALL_STATE(404)] = 10679,
  [SMALL_STATE(405)] = 10693,
  [SMALL_STATE(406)] = 10707,
  [SMALL_STATE(407)] = 10721,
  [SMALL_STATE(408)] = 10735,
  [SMALL_STATE(409)] = 10749,
  [SMALL_STATE(410)] = 10763,
  [SMALL_STATE(411)] = 10777,
  [SMALL_STATE(412)] = 10791,
  [SMALL_STATE(413)] = 10805,
  [SMALL_STATE(414)] = 10819,
  [SMALL_STATE(415)] = 10833,
  [SMALL_STATE(416)] = 10847,
  [SMALL_STATE(417)] = 10861,
  [SMALL_STATE(418)] = 10875,
  [SMALL_STATE(419)] = 10889,
  [SMALL_STATE(420)] = 10903,
  [SMALL_STATE(421)] = 10917,
};

static const TSParseActionEntry ts_parse_actions[] = {
  [0] = {.entry = {.count = 0, .reusable = false}},
  [1] = {.entry = {.count = 1, .reusable = false}}, RECOVER(),
  [3] = {.entry = {.count = 1, .reusable = true}}, SHIFT(321),
  [5] = {.entry = {.count = 1, .reusable = true}}, SHIFT_EXTRA(),
  [7] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_translation_unit, 0),
  [9] = {.entry = {.count = 1, .reusable = true}}, SHIFT(316),
  [11] = {.entry = {.count = 1, .reusable = true}}, SHIFT(29),
  [13] = {.entry = {.count = 1, .reusable = true}}, SHIFT(418),
  [15] = {.entry = {.count = 1, .reusable = true}}, SHIFT(312),
  [17] = {.entry = {.count = 1, .reusable = true}}, SHIFT(415),
  [19] = {.entry = {.count = 1, .reusable = true}}, SHIFT(412),
  [21] = {.entry = {.count = 1, .reusable = true}}, SHIFT(411),
  [23] = {.entry = {.count = 1, .reusable = true}}, SHIFT(233),
  [25] = {.entry = {.count = 1, .reusable = true}}, SHIFT(406),
  [27] = {.entry = {.count = 1, .reusable = true}}, SHIFT(176),
  [29] = {.entry = {.count = 1, .reusable = false}}, SHIFT(86),
  [31] = {.entry = {.count = 1, .reusable = false}}, SHIFT(100),
  [33] = {.entry = {.count = 1, .reusable = true}}, SHIFT(99),
  [35] = {.entry = {.count = 1, .reusable = false}}, SHIFT(99),
  [37] = {.entry = {.count = 1, .reusable = true}}, SHIFT(98),
  [39] = {.entry = {.count = 1, .reusable = false}}, SHIFT(98),
  [41] = {.entry = {.count = 1, .reusable = false}}, SHIFT(334),
  [43] = {.entry = {.count = 1, .reusable = false}}, SHIFT(325),
  [45] = {.entry = {.count = 1, .reusable = false}}, SHIFT(199),
  [47] = {.entry = {.count = 1, .reusable = false}}, SHIFT(307),
  [49] = {.entry = {.count = 1, .reusable = false}}, SHIFT(333),
  [51] = {.entry = {.count = 1, .reusable = false}}, SHIFT(200),
  [53] = {.entry = {.count = 1, .reusable = false}}, SHIFT(329),
  [55] = {.entry = {.count = 1, .reusable = false}}, SHIFT(328),
  [57] = {.entry = {.count = 1, .reusable = false}}, SHIFT(383),
  [59] = {.entry = {.count = 1, .reusable = false}}, SHIFT(191),
  [61] = {.entry = {.count = 1, .reusable = false}}, SHIFT(288),
  [63] = {.entry = {.count = 1, .reusable = false}}, SHIFT(344),
  [65] = {.entry = {.count = 1, .reusable = false}}, SHIFT(96),
  [67] = {.entry = {.count = 1, .reusable = true}}, SHIFT(43),
  [69] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_case_selectors, 3),
  [71] = {.entry = {.count = 1, .reusable = true}}, SHIFT(17),
  [73] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_case_selectors, 2),
  [75] = {.entry = {.count = 1, .reusable = true}}, SHIFT(135),
  [77] = {.entry = {.count = 1, .reusable = true}}, SHIFT(93),
  [79] = {.entry = {.count = 1, .reusable = true}}, SHIFT(161),
  [81] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_argument_expression_list_repeat1, 2), SHIFT_REPEAT(86),
  [84] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_argument_expression_list_repeat1, 2), SHIFT_REPEAT(100),
  [87] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_argument_expression_list_repeat1, 2), SHIFT_REPEAT(99),
  [90] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_argument_expression_list_repeat1, 2), SHIFT_REPEAT(99),
  [93] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_argument_expression_list_repeat1, 2), SHIFT_REPEAT(98),
  [96] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_argument_expression_list_repeat1, 2), SHIFT_REPEAT(98),
  [99] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_argument_expression_list_repeat1, 2), SHIFT_REPEAT(334),
  [102] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_argument_expression_list_repeat1, 2), SHIFT_REPEAT(325),
  [105] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_argument_expression_list_repeat1, 2), SHIFT_REPEAT(199),
  [108] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_argument_expression_list_repeat1, 2), SHIFT_REPEAT(307),
  [111] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_argument_expression_list_repeat1, 2), SHIFT_REPEAT(333),
  [114] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_argument_expression_list_repeat1, 2), SHIFT_REPEAT(200),
  [117] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_argument_expression_list_repeat1, 2), SHIFT_REPEAT(329),
  [120] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_argument_expression_list_repeat1, 2), SHIFT_REPEAT(328),
  [123] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_argument_expression_list_repeat1, 2), SHIFT_REPEAT(383),
  [126] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_argument_expression_list_repeat1, 2), SHIFT_REPEAT(191),
  [129] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_argument_expression_list_repeat1, 2), SHIFT_REPEAT(288),
  [132] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_argument_expression_list_repeat1, 2), SHIFT_REPEAT(344),
  [135] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_argument_expression_list_repeat1, 2), SHIFT_REPEAT(96),
  [138] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_argument_expression_list_repeat1, 2), SHIFT_REPEAT(43),
  [141] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_argument_expression_list_repeat1, 2), SHIFT_REPEAT(17),
  [144] = {.entry = {.count = 1, .reusable = true}}, SHIFT(157),
  [146] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_return_statement, 1),
  [148] = {.entry = {.count = 1, .reusable = false}}, SHIFT(134),
  [150] = {.entry = {.count = 1, .reusable = false}}, SHIFT(149),
  [152] = {.entry = {.count = 1, .reusable = true}}, SHIFT(137),
  [154] = {.entry = {.count = 1, .reusable = false}}, SHIFT(137),
  [156] = {.entry = {.count = 1, .reusable = true}}, SHIFT(144),
  [158] = {.entry = {.count = 1, .reusable = false}}, SHIFT(144),
  [160] = {.entry = {.count = 1, .reusable = false}}, SHIFT(420),
  [162] = {.entry = {.count = 1, .reusable = false}}, SHIFT(143),
  [164] = {.entry = {.count = 1, .reusable = true}}, SHIFT(37),
  [166] = {.entry = {.count = 1, .reusable = true}}, SHIFT(20),
  [168] = {.entry = {.count = 1, .reusable = true}}, SHIFT(40),
  [170] = {.entry = {.count = 1, .reusable = false}}, REDUCE(aux_sym_argument_expression_list_repeat1, 2),
  [172] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_argument_expression_list_repeat1, 2),
  [174] = {.entry = {.count = 1, .reusable = true}}, SHIFT(91),
  [176] = {.entry = {.count = 1, .reusable = true}}, SHIFT(97),
  [178] = {.entry = {.count = 1, .reusable = true}}, SHIFT(140),
  [180] = {.entry = {.count = 1, .reusable = true}}, SHIFT(139),
  [182] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_compound_assignment_operator, 1),
  [184] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_compound_assignment_operator, 1),
  [186] = {.entry = {.count = 1, .reusable = false}}, SHIFT(212),
  [188] = {.entry = {.count = 1, .reusable = false}}, REDUCE(aux_sym_struct_member_repeat1, 2),
  [190] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_struct_member_repeat1, 2), SHIFT_REPEAT(406),
  [193] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_compound_statement_repeat1, 2), SHIFT_REPEAT(153),
  [196] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_compound_statement_repeat1, 2), SHIFT_REPEAT(29),
  [199] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_compound_statement_repeat1, 2), SHIFT_REPEAT(416),
  [202] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_compound_statement_repeat1, 2), SHIFT_REPEAT(305),
  [205] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_compound_statement_repeat1, 2), SHIFT_REPEAT(413),
  [208] = {.entry = {.count = 1, .reusable = false}}, REDUCE(aux_sym_compound_statement_repeat1, 2),
  [210] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_compound_statement_repeat1, 2), SHIFT_REPEAT(408),
  [213] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_compound_statement_repeat1, 2), SHIFT_REPEAT(407),
  [216] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_compound_statement_repeat1, 2), SHIFT_REPEAT(13),
  [219] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_compound_statement_repeat1, 2), SHIFT_REPEAT(405),
  [222] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_compound_statement_repeat1, 2), SHIFT_REPEAT(12),
  [225] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_compound_statement_repeat1, 2), SHIFT_REPEAT(16),
  [228] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_compound_statement_repeat1, 2), SHIFT_REPEAT(233),
  [231] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_compound_statement_repeat1, 2), SHIFT_REPEAT(21),
  [234] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_compound_statement_repeat1, 2), SHIFT_REPEAT(235),
  [237] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_compound_statement_repeat1, 2), SHIFT_REPEAT(79),
  [240] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_compound_statement_repeat1, 2),
  [242] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_compound_statement_repeat1, 2), SHIFT_REPEAT(213),
  [245] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_compound_statement_repeat1, 2), SHIFT_REPEAT(108),
  [248] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_compound_statement_repeat1, 2), SHIFT_REPEAT(403),
  [251] = {.entry = {.count = 1, .reusable = false}}, SHIFT(153),
  [253] = {.entry = {.count = 1, .reusable = false}}, SHIFT(29),
  [255] = {.entry = {.count = 1, .reusable = false}}, SHIFT(416),
  [257] = {.entry = {.count = 1, .reusable = false}}, SHIFT(305),
  [259] = {.entry = {.count = 1, .reusable = false}}, SHIFT(413),
  [261] = {.entry = {.count = 1, .reusable = false}}, SHIFT(293),
  [263] = {.entry = {.count = 1, .reusable = false}}, SHIFT(408),
  [265] = {.entry = {.count = 1, .reusable = false}}, SHIFT(407),
  [267] = {.entry = {.count = 1, .reusable = false}}, SHIFT(13),
  [269] = {.entry = {.count = 1, .reusable = false}}, SHIFT(405),
  [271] = {.entry = {.count = 1, .reusable = false}}, SHIFT(12),
  [273] = {.entry = {.count = 1, .reusable = false}}, SHIFT(16),
  [275] = {.entry = {.count = 1, .reusable = false}}, SHIFT(233),
  [277] = {.entry = {.count = 1, .reusable = false}}, SHIFT(21),
  [279] = {.entry = {.count = 1, .reusable = true}}, SHIFT(235),
  [281] = {.entry = {.count = 1, .reusable = true}}, SHIFT(79),
  [283] = {.entry = {.count = 1, .reusable = true}}, SHIFT(114),
  [285] = {.entry = {.count = 1, .reusable = true}}, SHIFT(213),
  [287] = {.entry = {.count = 1, .reusable = true}}, SHIFT(108),
  [289] = {.entry = {.count = 1, .reusable = false}}, SHIFT(403),
  [291] = {.entry = {.count = 1, .reusable = true}}, SHIFT(106),
  [293] = {.entry = {.count = 1, .reusable = false}}, SHIFT(341),
  [295] = {.entry = {.count = 1, .reusable = true}}, SHIFT(253),
  [297] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_attribute, 2),
  [299] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_attribute, 2),
  [301] = {.entry = {.count = 1, .reusable = true}}, SHIFT(132),
  [303] = {.entry = {.count = 1, .reusable = true}}, SHIFT(259),
  [305] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_attribute, 7),
  [307] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_attribute, 7),
  [309] = {.entry = {.count = 1, .reusable = false}}, SHIFT(309),
  [311] = {.entry = {.count = 1, .reusable = true}}, SHIFT(335),
  [313] = {.entry = {.count = 1, .reusable = true}}, SHIFT(319),
  [315] = {.entry = {.count = 1, .reusable = false}}, REDUCE(aux_sym_struct_member_repeat1, 1),
  [317] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_struct_member_repeat1, 1),
  [319] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_attribute, 6),
  [321] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_attribute, 6),
  [323] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_attribute, 5),
  [325] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_attribute, 5),
  [327] = {.entry = {.count = 1, .reusable = true}}, SHIFT(80),
  [329] = {.entry = {.count = 1, .reusable = true}}, SHIFT(81),
  [331] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_compound_statement, 3),
  [333] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_compound_statement, 3),
  [335] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_compound_statement, 2),
  [337] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_compound_statement, 2),
  [339] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_singular_expression, 1),
  [341] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_singular_expression, 1),
  [343] = {.entry = {.count = 1, .reusable = true}}, SHIFT(25),
  [345] = {.entry = {.count = 1, .reusable = true}}, SHIFT(419),
  [347] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_postfix_expression, 3),
  [349] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_postfix_expression, 3),
  [351] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_postfix_expression, 2),
  [353] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_postfix_expression, 2),
  [355] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_translation_unit, 1),
  [357] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_primary_expression, 1),
  [359] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_primary_expression, 1),
  [361] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_callable, 1),
  [363] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_float_literal, 1),
  [365] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_float_literal, 1),
  [367] = {.entry = {.count = 1, .reusable = true}}, SHIFT(19),
  [369] = {.entry = {.count = 1, .reusable = true}}, SHIFT(399),
  [371] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_if_statement, 3),
  [373] = {.entry = {.count = 1, .reusable = false}}, SHIFT(226),
  [375] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_if_statement, 3),
  [377] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_primary_expression, 5),
  [379] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_primary_expression, 5),
  [381] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_argument_expression_list, 4),
  [383] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_argument_expression_list, 4),
  [385] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_argument_expression_list, 2),
  [387] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_argument_expression_list, 2),
  [389] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_const_literal, 1),
  [391] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_const_literal, 1),
  [393] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_paren_expression, 3),
  [395] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_paren_expression, 3),
  [397] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_bool_literal, 1),
  [399] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_bool_literal, 1),
  [401] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_argument_expression_list, 5),
  [403] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_argument_expression_list, 5),
  [405] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_hex_float_literal, 1),
  [407] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_hex_float_literal, 1),
  [409] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_decimal_float_literal, 1),
  [411] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_decimal_float_literal, 1),
  [413] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_int_literal, 1),
  [415] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_int_literal, 1),
  [417] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_argument_expression_list, 3),
  [419] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_argument_expression_list, 3),
  [421] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_primary_expression, 2),
  [423] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_primary_expression, 2),
  [425] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_else_statement, 1),
  [427] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_else_statement, 1),
  [429] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_for_statement, 5),
  [431] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_for_statement, 5),
  [433] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_loop_statement, 3),
  [435] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_loop_statement, 3),
  [437] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_switch_statement, 5),
  [439] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_switch_statement, 5),
  [441] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_statement, 1),
  [443] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_statement, 1),
  [445] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_while_statement, 3),
  [447] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_while_statement, 3),
  [449] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_if_statement, 5),
  [451] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_if_statement, 5),
  [453] = {.entry = {.count = 1, .reusable = false}}, REDUCE(aux_sym_compound_statement_repeat1, 1),
  [455] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_compound_statement_repeat1, 1),
  [457] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_loop_statement, 5),
  [459] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_loop_statement, 5),
  [461] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_statement, 2),
  [463] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_statement, 2),
  [465] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_loop_statement, 4),
  [467] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_loop_statement, 4),
  [469] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_postfix_expression, 4),
  [471] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_postfix_expression, 4),
  [473] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_unary_expression, 1),
  [475] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_unary_expression, 1),
  [477] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_singular_expression, 2),
  [479] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_singular_expression, 2),
  [481] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_unary_expression, 2),
  [483] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_unary_expression, 2),
  [485] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_binary_and_expression, 1),
  [487] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_multiplicative_expression, 1),
  [489] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_multiplicative_expression, 1),
  [491] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_binary_or_expression, 1),
  [493] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_binary_xor_expression, 1),
  [495] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_translation_unit_repeat2, 2),
  [497] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_translation_unit_repeat2, 2), SHIFT_REPEAT(316),
  [500] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_translation_unit_repeat2, 2), SHIFT_REPEAT(29),
  [503] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_translation_unit_repeat2, 2), SHIFT_REPEAT(418),
  [506] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_translation_unit_repeat2, 2), SHIFT_REPEAT(312),
  [509] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_translation_unit_repeat2, 2), SHIFT_REPEAT(412),
  [512] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_translation_unit_repeat2, 2), SHIFT_REPEAT(411),
  [515] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_translation_unit_repeat2, 2), SHIFT_REPEAT(233),
  [518] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_translation_unit_repeat2, 2), SHIFT_REPEAT(406),
  [521] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_translation_unit_repeat2, 2), SHIFT_REPEAT(176),
  [524] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_translation_unit, 2),
  [526] = {.entry = {.count = 1, .reusable = true}}, SHIFT(6),
  [528] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_multiplicative_expression, 3),
  [530] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_multiplicative_expression, 3),
  [532] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_additive_expression, 3),
  [534] = {.entry = {.count = 1, .reusable = false}}, SHIFT(46),
  [536] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_additive_expression, 3),
  [538] = {.entry = {.count = 1, .reusable = true}}, SHIFT(46),
  [540] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_additive_expression, 1),
  [542] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_additive_expression, 1),
  [544] = {.entry = {.count = 1, .reusable = true}}, SHIFT(276),
  [546] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_attribute_repeat1, 2), SHIFT_REPEAT(276),
  [549] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_attribute_repeat1, 2), SHIFT_REPEAT(100),
  [552] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_attribute_repeat1, 2), SHIFT_REPEAT(99),
  [555] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_attribute_repeat1, 2), SHIFT_REPEAT(99),
  [558] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_attribute_repeat1, 2), SHIFT_REPEAT(98),
  [561] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_attribute_repeat1, 2), SHIFT_REPEAT(98),
  [564] = {.entry = {.count = 1, .reusable = true}}, SHIFT(392),
  [566] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_relational_expression, 1),
  [568] = {.entry = {.count = 1, .reusable = true}}, SHIFT(34),
  [570] = {.entry = {.count = 1, .reusable = false}}, SHIFT(34),
  [572] = {.entry = {.count = 1, .reusable = true}}, SHIFT(35),
  [574] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_core_lhs_expression, 1),
  [576] = {.entry = {.count = 1, .reusable = true}}, SHIFT(5),
  [578] = {.entry = {.count = 1, .reusable = true}}, SHIFT(153),
  [580] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_for_header, 4),
  [582] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_for_header, 3),
  [584] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_translation_unit_repeat1, 2),
  [586] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_translation_unit_repeat1, 2), SHIFT_REPEAT(415),
  [589] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_lhs_expression, 1),
  [591] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_core_lhs_expression, 3),
  [593] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_for_header, 2),
  [595] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_lhs_expression, 2),
  [597] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_attribute_repeat1, 2),
  [599] = {.entry = {.count = 1, .reusable = false}}, REDUCE(aux_sym_attribute_repeat1, 2),
  [601] = {.entry = {.count = 1, .reusable = true}}, SHIFT(76),
  [603] = {.entry = {.count = 1, .reusable = true}}, SHIFT(72),
  [605] = {.entry = {.count = 1, .reusable = true}}, SHIFT(14),
  [607] = {.entry = {.count = 1, .reusable = true}}, SHIFT(266),
  [609] = {.entry = {.count = 1, .reusable = true}}, SHIFT(265),
  [611] = {.entry = {.count = 1, .reusable = true}}, SHIFT(51),
  [613] = {.entry = {.count = 1, .reusable = true}}, SHIFT(15),
  [615] = {.entry = {.count = 1, .reusable = true}}, SHIFT(409),
  [617] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_lhs_expression, 3),
  [619] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_translation_unit_repeat1, 1),
  [621] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_enable_directive, 3),
  [623] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_global_directive, 1),
  [625] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_translation_unit_repeat2, 1),
  [627] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_relational_expression, 3),
  [629] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_function_decl, 2),
  [631] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_struct_decl, 3),
  [633] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_function_decl, 3),
  [635] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_struct_body_decl, 5),
  [637] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_struct_body_decl, 4),
  [639] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_struct_body_decl, 3),
  [641] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_binary_and_expression, 1),
  [643] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_binary_or_expression, 1),
  [645] = {.entry = {.count = 1, .reusable = true}}, SHIFT(389),
  [647] = {.entry = {.count = 1, .reusable = true}}, SHIFT(279),
  [649] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_short_circuit_and_expression, 1),
  [651] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_expression, 1),
  [653] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_short_circuit_or_expression, 1),
  [655] = {.entry = {.count = 1, .reusable = true}}, SHIFT(268),
  [657] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_depth_texture_type, 1),
  [659] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_texture_sampler_types, 6),
  [661] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_texture_sampler_types, 4),
  [663] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_type_decl_without_ident, 6),
  [665] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_type_decl_without_ident, 4),
  [667] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_array_type_decl, 4),
  [669] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_bitwise_expression, 3),
  [671] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_binary_xor_expression, 3),
  [673] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_binary_and_expression, 3),
  [675] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_type_decl_without_ident, 1),
  [677] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_sampler_type, 1),
  [679] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_binary_or_expression, 3),
  [681] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_array_type_decl, 6),
  [683] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_texture_sampler_types, 1),
  [685] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_type_decl_without_ident, 8),
  [687] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_struct_body_decl_repeat1, 2), SHIFT_REPEAT(389),
  [690] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_struct_body_decl_repeat1, 2), SHIFT_REPEAT(406),
  [693] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_short_circuit_and_expression, 3),
  [695] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_expression, 3),
  [697] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_short_circuit_or_expression, 3),
  [699] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_param_list_repeat1, 2), SHIFT_REPEAT(389),
  [702] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_param_list_repeat1, 2), SHIFT_REPEAT(406),
  [705] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_type_decl, 1),
  [707] = {.entry = {.count = 1, .reusable = true}}, SHIFT(155),
  [709] = {.entry = {.count = 1, .reusable = false}}, SHIFT(41),
  [711] = {.entry = {.count = 1, .reusable = true}}, SHIFT(41),
  [713] = {.entry = {.count = 1, .reusable = true}}, SHIFT(311),
  [715] = {.entry = {.count = 1, .reusable = true}}, SHIFT(10),
  [717] = {.entry = {.count = 1, .reusable = true}}, SHIFT(249),
  [719] = {.entry = {.count = 1, .reusable = true}}, SHIFT(107),
  [721] = {.entry = {.count = 1, .reusable = true}}, SHIFT(13),
  [723] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_switch_statement_repeat1, 2), SHIFT_REPEAT(10),
  [726] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_switch_statement_repeat1, 2), SHIFT_REPEAT(249),
  [729] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_switch_statement_repeat1, 2),
  [731] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_lhs_expression_repeat1, 2),
  [733] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_lhs_expression_repeat1, 2), SHIFT_REPEAT(235),
  [736] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_variable_ident_decl, 3),
  [738] = {.entry = {.count = 1, .reusable = false}}, SHIFT(337),
  [740] = {.entry = {.count = 1, .reusable = true}}, SHIFT(337),
  [742] = {.entry = {.count = 1, .reusable = true}}, SHIFT(251),
  [744] = {.entry = {.count = 1, .reusable = true}}, SHIFT(221),
  [746] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_case_selectors_repeat1, 2),
  [748] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_case_selectors_repeat1, 2), SHIFT_REPEAT(22),
  [751] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_lhs_expression_repeat1, 1),
  [753] = {.entry = {.count = 1, .reusable = true}}, SHIFT(2),
  [755] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_case_selectors, 1),
  [757] = {.entry = {.count = 1, .reusable = true}}, SHIFT(3),
  [759] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_element_count_expression, 1),
  [761] = {.entry = {.count = 1, .reusable = true}}, SHIFT(36),
  [763] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_switch_body, 2),
  [765] = {.entry = {.count = 1, .reusable = true}}, SHIFT(69),
  [767] = {.entry = {.count = 1, .reusable = true}}, SHIFT(284),
  [769] = {.entry = {.count = 1, .reusable = true}}, SHIFT(58),
  [771] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_variable_decl, 3),
  [773] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_param_list_repeat1, 2),
  [775] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_param_list, 2),
  [777] = {.entry = {.count = 1, .reusable = true}}, SHIFT(26),
  [779] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_global_constant_decl, 3),
  [781] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_switch_statement_repeat1, 1),
  [783] = {.entry = {.count = 1, .reusable = true}}, SHIFT(314),
  [785] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_struct_body_decl_repeat1, 2),
  [787] = {.entry = {.count = 1, .reusable = true}}, SHIFT(183),
  [789] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_variable_decl, 2),
  [791] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_switch_body, 3),
  [793] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_case_compound_statement, 2),
  [795] = {.entry = {.count = 1, .reusable = true}}, SHIFT(24),
  [797] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_global_constant_decl, 2),
  [799] = {.entry = {.count = 1, .reusable = true}}, SHIFT(182),
  [801] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_switch_body, 4),
  [803] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_case_compound_statement, 4),
  [805] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_param_list, 3),
  [807] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_case_compound_statement, 3),
  [809] = {.entry = {.count = 1, .reusable = true}}, SHIFT(23),
  [811] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_global_variable_decl, 1),
  [813] = {.entry = {.count = 1, .reusable = true}}, SHIFT(255),
  [815] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_assignment_statement, 3),
  [817] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_increment_statement, 2),
  [819] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_decrement_statement, 2),
  [821] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_struct_member, 2),
  [823] = {.entry = {.count = 1, .reusable = true}}, SHIFT(247),
  [825] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_func_call_statement, 2),
  [827] = {.entry = {.count = 1, .reusable = true}}, SHIFT(232),
  [829] = {.entry = {.count = 1, .reusable = true}}, SHIFT(194),
  [831] = {.entry = {.count = 1, .reusable = true}}, SHIFT(59),
  [833] = {.entry = {.count = 1, .reusable = true}}, SHIFT(18),
  [835] = {.entry = {.count = 1, .reusable = true}}, SHIFT(164),
  [837] = {.entry = {.count = 1, .reusable = true}}, SHIFT(77),
  [839] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_literal_or_ident, 1),
  [841] = {.entry = {.count = 1, .reusable = true}}, SHIFT(246),
  [843] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_param_list, 1),
  [845] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_param, 1),
  [847] = {.entry = {.count = 1, .reusable = true}}, SHIFT(53),
  [849] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_function_header, 4),
  [851] = {.entry = {.count = 1, .reusable = true}}, SHIFT(165),
  [853] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_struct_member, 1),
  [855] = {.entry = {.count = 1, .reusable = true}}, SHIFT(48),
  [857] = {.entry = {.count = 1, .reusable = true}}, SHIFT(184),
  [859] = {.entry = {.count = 1, .reusable = true}}, SHIFT(250),
  [861] = {.entry = {.count = 1, .reusable = true}}, SHIFT(47),
  [863] = {.entry = {.count = 1, .reusable = true}}, SHIFT(101),
  [865] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_vec_prefix, 1),
  [867] = {.entry = {.count = 1, .reusable = true}}, SHIFT(28),
  [869] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_global_variable_decl, 2),
  [871] = {.entry = {.count = 1, .reusable = true}}, SHIFT(245),
  [873] = {.entry = {.count = 1, .reusable = true}}, SHIFT(74),
  [875] = {.entry = {.count = 1, .reusable = true}}, SHIFT(4),
  [877] = {.entry = {.count = 1, .reusable = true}}, SHIFT(31),
  [879] = {.entry = {.count = 1, .reusable = true}}, SHIFT(196),
  [881] = {.entry = {.count = 1, .reusable = true}}, SHIFT(258),
  [883] = {.entry = {.count = 1, .reusable = true}}, SHIFT(27),
  [885] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_variable_statement, 1),
  [887] = {.entry = {.count = 1, .reusable = true}}, SHIFT(50),
  [889] = {.entry = {.count = 1, .reusable = true}}, SHIFT(136),
  [891] = {.entry = {.count = 1, .reusable = true}}, SHIFT(49),
  [893] = {.entry = {.count = 1, .reusable = true}}, SHIFT(274),
  [895] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_mat_prefix, 1),
  [897] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_param, 2),
  [899] = {.entry = {.count = 1, .reusable = true}}, SHIFT(30),
  [901] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_break_statement, 1),
  [903] = {.entry = {.count = 1, .reusable = true}}, SHIFT(231),
  [905] = {.entry = {.count = 1, .reusable = true}}, SHIFT(386),
  [907] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_address_space, 1),
  [909] = {.entry = {.count = 1, .reusable = true}}, SHIFT(262),
  [911] = {.entry = {.count = 1, .reusable = true}}, SHIFT(54),
  [913] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_function_header, 5),
  [915] = {.entry = {.count = 1, .reusable = true}}, SHIFT(207),
  [917] = {.entry = {.count = 1, .reusable = true}}, SHIFT(254),
  [919] = {.entry = {.count = 1, .reusable = true}}, SHIFT(202),
  [921] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_continuing_compound_statement, 2),
  [923] = {.entry = {.count = 1, .reusable = false}}, SHIFT(321),
  [925] = {.entry = {.count = 1, .reusable = false}}, SHIFT(421),
  [927] = {.entry = {.count = 1, .reusable = false}}, SHIFT_EXTRA(),
  [929] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_for_update, 1),
  [931] = {.entry = {.count = 1, .reusable = true}}, SHIFT(112),
  [933] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_function_header, 7),
  [935] = {.entry = {.count = 1, .reusable = true}}, SHIFT(156),
  [937] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_multisampled_texture_type, 1),
  [939] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_sampled_texture_type, 1),
  [941] = {.entry = {.count = 1, .reusable = true}}, SHIFT(204),
  [943] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_function_header, 8),
  [945] = {.entry = {.count = 1, .reusable = true}}, SHIFT(217),
  [947] = {.entry = {.count = 1, .reusable = true}}, SHIFT(60),
  [949] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_continuing_compound_statement, 3),
  [951] = {.entry = {.count = 1, .reusable = true}}, SHIFT(348),
  [953] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_access_mode, 1),
  [955] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_continuing_statement, 2),
  [957] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_variable_statement, 4),
  [959] = {.entry = {.count = 1, .reusable = true}}, SHIFT(352),
  [961] = {.entry = {.count = 1, .reusable = true}}, SHIFT(172),
  [963] = {.entry = {.count = 1, .reusable = true}}, SHIFT(57),
  [965] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_variable_qualifier, 5),
  [967] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_for_header, 5),
  [969] = {.entry = {.count = 1, .reusable = true}}, SHIFT(355),
  [971] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_continuing_compound_statement, 4),
  [973] = {.entry = {.count = 1, .reusable = true}}, SHIFT(303),
  [975] = {.entry = {.count = 1, .reusable = true}}, SHIFT(64),
  [977] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_function_header, 6),
  [979] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_fallthrough_statement, 2),
  [981] = {.entry = {.count = 1, .reusable = true}}, SHIFT(301),
  [983] = {.entry = {.count = 1, .reusable = true}}, SHIFT(257),
  [985] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_break_if_statement, 4),
  [987] = {.entry = {.count = 1, .reusable = true}}, SHIFT(52),
  [989] = {.entry = {.count = 1, .reusable = true}}, SHIFT(138),
  [991] = {.entry = {.count = 1, .reusable = true}}, SHIFT(192),
  [993] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_global_constant_decl, 5),
  [995] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_variable_statement, 3),
  [997] = {.entry = {.count = 1, .reusable = true}}, SHIFT(9),
  [999] = {.entry = {.count = 1, .reusable = true}}, SHIFT(287),
  [1001] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_for_init, 1),
  [1003] = {.entry = {.count = 1, .reusable = true}}, SHIFT(345),
  [1005] = {.entry = {.count = 1, .reusable = true}}, SHIFT(95),
  [1007] = {.entry = {.count = 1, .reusable = true}}, SHIFT(32),
  [1009] = {.entry = {.count = 1, .reusable = true}}, SHIFT(33),
  [1011] = {.entry = {.count = 1, .reusable = true}}, SHIFT(42),
  [1013] = {.entry = {.count = 1, .reusable = true}}, SHIFT(39),
  [1015] = {.entry = {.count = 1, .reusable = true}}, SHIFT(38),
  [1017] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_global_variable_decl, 4),
  [1019] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_static_assert_statement, 2),
  [1021] = {.entry = {.count = 1, .reusable = true}}, SHIFT(180),
  [1023] = {.entry = {.count = 1, .reusable = true}}, SHIFT(174),
  [1025] = {.entry = {.count = 1, .reusable = true}}, SHIFT(188),
  [1027] = {.entry = {.count = 1, .reusable = true}}, SHIFT(160),
  [1029] = {.entry = {.count = 1, .reusable = true}}, SHIFT(237),
  [1031] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_return_statement, 2),
  [1033] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_storage_texture_type, 1),
  [1035] = {.entry = {.count = 1, .reusable = true}},  ACCEPT_INPUT(),
  [1037] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_variable_qualifier, 3),
  [1039] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_type_alias_decl, 4),
  [1041] = {.entry = {.count = 1, .reusable = true}}, SHIFT(313),
  [1043] = {.entry = {.count = 1, .reusable = true}}, SHIFT(83),
  [1045] = {.entry = {.count = 1, .reusable = true}}, SHIFT(236),
  [1047] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_texel_format, 1),
  [1049] = {.entry = {.count = 1, .reusable = true}}, SHIFT(193),
  [1051] = {.entry = {.count = 1, .reusable = true}}, SHIFT(294),
  [1053] = {.entry = {.count = 1, .reusable = true}}, SHIFT(63),
  [1055] = {.entry = {.count = 1, .reusable = true}}, SHIFT(44),
  [1057] = {.entry = {.count = 1, .reusable = true}}, SHIFT(45),
  [1059] = {.entry = {.count = 1, .reusable = true}}, SHIFT(145),
  [1061] = {.entry = {.count = 1, .reusable = true}}, SHIFT(102),
  [1063] = {.entry = {.count = 1, .reusable = true}}, SHIFT(195),
  [1065] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_global_constant_decl, 4),
  [1067] = {.entry = {.count = 1, .reusable = true}}, SHIFT(88),
  [1069] = {.entry = {.count = 1, .reusable = true}}, SHIFT(62),
  [1071] = {.entry = {.count = 1, .reusable = true}}, SHIFT(68),
  [1073] = {.entry = {.count = 1, .reusable = true}}, SHIFT(70),
  [1075] = {.entry = {.count = 1, .reusable = true}}, SHIFT(126),
  [1077] = {.entry = {.count = 1, .reusable = true}}, SHIFT(113),
  [1079] = {.entry = {.count = 1, .reusable = true}}, SHIFT(167),
  [1081] = {.entry = {.count = 1, .reusable = true}}, SHIFT(170),
  [1083] = {.entry = {.count = 1, .reusable = true}}, SHIFT(404),
  [1085] = {.entry = {.count = 1, .reusable = true}}, SHIFT(379),
  [1087] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_continue_statement, 1),
  [1089] = {.entry = {.count = 1, .reusable = true}}, SHIFT(302),
  [1091] = {.entry = {.count = 1, .reusable = true}}, SHIFT(378),
  [1093] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_global_variable_decl, 3),
  [1095] = {.entry = {.count = 1, .reusable = true}}, SHIFT(315),
  [1097] = {.entry = {.count = 1, .reusable = true}}, SHIFT(84),
  [1099] = {.entry = {.count = 1, .reusable = true}}, SHIFT(61),
  [1101] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym__comment, 2),
};

#ifdef __cplusplus
extern "C" {
#endif
void *tree_sitter_wgsl_external_scanner_create(void);
void tree_sitter_wgsl_external_scanner_destroy(void *);
bool tree_sitter_wgsl_external_scanner_scan(void *, TSLexer *, const bool *);
unsigned tree_sitter_wgsl_external_scanner_serialize(void *, char *);
void tree_sitter_wgsl_external_scanner_deserialize(void *, const char *, unsigned);

#ifdef _WIN32
#define extern __declspec(dllexport)
#endif

extern const TSLanguage *tree_sitter_wgsl(void) {
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
    .keyword_lex_fn = ts_lex_keywords,
    .keyword_capture_token = sym_ident,
    .external_scanner = {
      &ts_external_scanner_states[0][0],
      ts_external_scanner_symbol_map,
      tree_sitter_wgsl_external_scanner_create,
      tree_sitter_wgsl_external_scanner_destroy,
      tree_sitter_wgsl_external_scanner_scan,
      tree_sitter_wgsl_external_scanner_serialize,
      tree_sitter_wgsl_external_scanner_deserialize,
    },
  };
  return &language;
}
#ifdef __cplusplus
}
#endif
