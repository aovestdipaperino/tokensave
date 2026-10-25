// Language reference (does not always work):
// https://help.adobe.com/en_US/FlashPlatform/reference/actionscript/3/language-elements.html

// https://help.adobe.com/en_US/as3/learn/WS597e5dadb9cc1e02-6e28df0f130d240d5d7-8000.html
const PREC = {
  PRIMARY: 16, // [] {x:y} () f(x) new x.y x[y] <></> @ :: ..
  POSTFIX: 15, // x++ x--
  UNARY: 14, // ++x --x + - ~ ! delete typeof void
  MULTIPLICATIVE: 13, // * / %
  ADDITIVE: 12, // + -
  BITWISE_SHIFT: 11, // << >> >>>
  RELATIONAL: 10, // < > <= >= as in instanceof is
  EQUALITY: 9, // == != === !==
  BITWISE_AND: 8, // &
  BITWISE_XOR: 7, // ^
  BITWISE_OR: 6, // |
  LOGICAL_AND: 5, // &&
  LOGICAL_OR: 4, // ||
  CONDITIONAL: 3, // ?:
  ASSIGNEMENT: 2, // = *= /= %= += -= <<= >>= >>>= &= ^= |=
  COMMA: 1, // ,
};

module.exports = grammar({
  name: 'actionscript',

  extras: ($) => [$.line_comment, $.block_comment, /\s/],

  supertypes: ($) => [
    $.statement,
    $.declaration,
    $.expression,
    $.primary_expression,
  ],

  conflicts: ($) => [
    [$.sequence_expression, $.pair],
    [$.annotation, $.primary_expression],
    [$.class_attribut, $.property_attribut],
    [$.primary_expression, $.for_in_statement],
    [$.primary_expression, $.for_each_in_statement],
  ],

  word: ($) => $.identifier,

  inline: ($) => [$._expressions, $.statement],

  rules: {
    program: ($) => repeat($.statement),

    // Declarations

    declaration: ($) =>
    choice(
      $.variable_declaration,
      $.constant_declaration,
      $.function_declaration,
      $.class_declaration,
      $.interface_declaration,
      $.package_declaration,
      $.namespace_declaration
    ),

    namespace_declaration: ($) =>
    seq(
      repeat($.annotation),
      repeat($.property_attribut),
      'namespace',
      field('name', $.identifier),
      optional(seq('=', field('value', $.expression))),
      ';'
    ),
    package_declaration: ($) =>
    seq(
      'package',
      optional(field('name', $._data_type)),
      field('body', $.statement_block)
    ),

    class_declaration: ($) =>
    seq(
      repeat($.annotation),
      repeat($.class_attribut),
      'class',
      field('name', $._data_type),
      optional(field('superclass', seq('extends', $._data_type))),
      optional(
        field('interfaces', seq('implements', sep1($._data_type, ',')))
      ),
      field('body', $.statement)
    ),

    annotation: ($) =>
    seq(
      '[',
      sep1(
        seq($.identifier, optional(seq('(', sep1($.expression, ','), ')'))),
        ','
      ),
      ']'
    ),

    // https://help.adobe.com/fr_FR/as3/learn/WS5b3ccc516d4fbf351e63e3d118a9b90204-7f36.html
    class_attribut: ($) => choice('dynamic', 'final', 'internal', 'public'),

    interface_declaration: ($) =>
    seq(
      repeat($.annotation),
      optional($.interface_attribut),
      'interface',
      field('name', $.identifier),
      optional(field('supertype', seq('extends', sep1($._data_type, ',')))),
      '{',
      field('body', repeat($.method_declaration)),
      '}'
    ),

    //https://help.adobe.com/en_US/as3/learn/WS5b3ccc516d4fbf351e63e3d118a9b90204-7f41.html
    interface_attribut: ($) => choice('internal', 'public'),

    method_declaration: ($) =>
    seq(
      repeat($.annotation),
      'function',
      optional($.accessor),
      field('name', choice('get', 'set', $.identifier)),
      '(',
      field('parameters', optional($.function_parameters)),
      ')',
      optional(field('return_type', $.type_hint)),
      ';'
    ),

    function_declaration: ($) =>
    seq(
      repeat($.annotation),
      repeat($.property_attribut),
      'function',
      optional($.accessor),
      // awful way to allow function named 'get' or 'set'
      field('name', choice('get', 'set', $.identifier)),
      '(',
      field('parameters', optional($.function_parameters)),
      ')',
      optional(field('return_type', $.type_hint)),
      field('body', $.statement_block)
    ),

    function_parameters: ($) =>
    sep1(
      choice(
        seq(
          $.identifier,
          optional(field('type', $.type_hint)),
          optional(field('default', seq('=', $.expression)))
        ),
        $.rest
      ),
      ','
    ),

    variable_declaration: ($) =>
    seq(
      repeat($.annotation),
      repeat($.property_attribut),
      $._variable_declarator,
      ';'
    ),

    _variable_declarator: ($) =>
    seq(
      'var',
      field('name', $.identifier),
      optional(field('type', $.type_hint)),
      optional(seq('=', field('value', $.expression)))
    ),

    constant_declaration: ($) =>
    seq(
      repeat($.annotation),
      repeat($.property_attribut),
      'const',
      field('name', $.identifier),
      optional(field('type', $.type_hint)),
      optional(seq('=', field('value', $.expression))),
      ';'
    ),

    // https://help.adobe.com/fr_FR/as3/learn/WS5b3ccc516d4fbf351e63e3d118a9b90204-7f36.html
    property_attribut: ($) =>
    choice(
      'internal',
      'private',
      'protected',
      'public',
      'static',
      'final',
      'override',
      $.identifier // namespace
    ),

    accessor: ($) => choice('get', 'set'),

    rest: ($) => seq('...', $.identifier),

    // Statements

    statement: ($) =>
    choice(
      $.import_statement,
      $.declaration,
      $.expression_statement,
      $.statement_block,
      $.if_statement,
      $.switch_statement,
      $.for_statement,
      $.for_in_statement,
      $.for_each_in_statement,
      $.while_statement,
      $.do_statement,
      $.try_statement,
      $.with_statement,
      $.break_statement,
      $.continue_statement,
      $.return_statement,
      $.throw_statement,
      $.empty_statement,
      $.labeled_statement
    ),

    import_statement: ($) => seq('import', $._data_type, ';'),

    expression_statement: ($) => seq($._expressions, ';'),

    statement_block: ($) => seq('{', repeat($.statement), '}'),

    if_statement: ($) =>
    prec.right(
      seq(
        'if',
        field('condition', $.parenthesized_expression),
        field('consequence', $.statement),
        optional(field('alternative', $.else_clause))
      )
    ),

    else_clause: ($) => seq('else', $.statement),

    switch_statement: ($) =>
    seq(
      'switch',
      field('value', $.parenthesized_expression),
      field('body', $.switch_body)
    ),
    switch_body: ($) =>
    seq('{', repeat(choice($.switch_case, $.switch_default)), '}'),
    switch_case: ($) =>
    seq(
      'case',
      field('value', $.expression),
      ':',
      field('body', repeat($.statement))
    ),
    switch_default: ($) =>
    seq('default', ':', field('body', repeat($.statement))),

    for_statement: ($) =>
    seq(
      'for',
      '(',
      field(
        'initializer',
        choice(
          seq(sep1($._variable_declarator, ','), ';'),
          $.expression_statement,
          $.empty_statement
        )
      ),
      field('condition', choice($.expression_statement, $.empty_statement)),
      field('increment', optional($._expressions)),
      ')',
      field('body', $.statement)
    ),

    for_in_statement: ($) =>
    seq(
      'for',
      '(',
      optional('var'),
      field(
        'left',
        seq(
          choice($.identifier, $.subscript_expression),
          optional($.type_hint)
        )
      ),
      'in',
      field('right', $.expression),
      ')',
      field('body', $.statement)
    ),

    for_each_in_statement: ($) =>
    seq(
      'for each',
      '(',
      optional('var'),
      field('left', seq($.identifier, optional($.type_hint))),
      'in',
      field('right', $.expression),
      ')',
      field('body', $.statement)
    ),

    while_statement: ($) =>
    seq(
      'while',
      field('condition', $.parenthesized_expression),
      field('body', $.statement)
    ),

    do_statement: ($) =>
    seq(
      'do',
      field('body', $.statement),
      'while',
      field('condition', $.parenthesized_expression),
      ';'
    ),

    with_statement: ($) =>
    seq(
      'with',
      field('object', $.parenthesized_expression),
      field('body', $.statement)
    ),

    try_statement: ($) =>
    seq(
      'try',
      field('body', $.statement_block),
      field('handler', repeat($.catch_clause)),
      optional(field('finalizer', $.finally_clause))
    ),
    catch_clause: ($) =>
    seq(
      'catch',
      '(',
      field('parameter', $.identifier),
      optional(field('type', $.type_hint)),
      ')',
      field('body', $.statement_block)
    ),
    finally_clause: ($) => seq('finally', field('body', $.statement_block)),

    break_statement: ($) =>
    seq('break', optional(field('label', $.identifier)), ';'),

    continue_statement: ($) =>
    seq('continue', optional(field('label', $.identifier)), ';'),

    return_statement: ($) => seq('return', optional($.expression), ';'),

    throw_statement: ($) => seq('throw', optional($.expression), ';'),

    empty_statement: ($) => ';',

    labeled_statement: ($) =>
    prec.dynamic(
      -1,
      seq(field('label', $.identifier), ':', field('body', $.statement))
    ),

    // Expressions

    _expressions: ($) => choice($.expression, $.sequence_expression),

    sequence_expression: ($) =>
    seq(
      field('left', $.expression),
      ',',
      field('right', choice($.expression, $.sequence_expression))
    ),

    expression: ($) =>
    choice(
      $.primary_expression,
      $.assignment_expression,
      $.augmented_assignment_expression,
      $.unary_expression,
      $.binary_expression,
      $.ternary_expression,
      $.update_expression,
      $.cast_expression,
      $.new_expression
    ),

    primary_expression: ($) =>
    choice(
      $.subscript_expression,
      $.member_expression,
      $.descendant_expression,
      $.namespace_expression,
      $.parenthesized_expression,
      $.identifier,
      $.regex,
      $.true,
      $.false,
      $.undefined,
      $.null,
      $.number,
      $.string,
      $.object,
      $.array,
      $.vector,
      $.xml,
      $.anonymous_function,
      $.call_expression,
      $.generic_data_type
    ),

    subscript_expression: ($) =>
    prec(
      PREC.PRIMARY,
      seq(
        field('object', choice($.expression)),
        '[',
        field('index', $.expression),
        ']'
      )
    ),

    member_expression: ($) =>
    prec(
      PREC.PRIMARY,
      seq(
        field('object', choice($.expression)),
        '.',
        field(
          'property',
          choice($.identifier, '*', $.parenthesized_expression)
        )
      )
    ),

    descendant_expression: ($) =>
    prec(
      PREC.PRIMARY,
      seq(
        field('object', choice($.expression)),
        '..',
        field('property', $.identifier)
      )
    ),

    namespace_expression: ($) =>
    prec(
      PREC.PRIMARY,
      seq(
        field('object', choice($.expression)),
        '::',
        field('property', $.identifier)
      )
    ),

    parenthesized_expression: ($) =>
    prec(PREC.PRIMARY, seq('(', $._expressions, ')')),

    object: ($) =>
    prec(PREC.PRIMARY, seq('{', optional(sep1($.pair, ',')), '}')),

    pair: ($) =>
    seq(
      field('key', choice($.identifier, $.string, $.number)),
      ':',
      field('value', $.expression)
    ),

    array: ($) =>
    prec(PREC.PRIMARY, seq('[', optional(sep1($.expression, ',')), ']')),

    vector: ($) =>
    prec(
      PREC.PRIMARY,
      // TODO: replace '$.identifier' with the more correct '$._data_type'
      seq('<', field('type', $.identifier), '>', field('value', $.array))
    ),

    xml: ($) =>
    choice(
      seq(
        '<',
        $.identifier,
        repeat($.xml_attribute),
        '>',
        // TODO: make a proper regex for the xml content
        choice(repeat($.xml), /[\w ]+/),
        '</',
        $.identifier,
        '>'
      ),
      seq('<', $.identifier, repeat($.xml_attribute), '/>')
    ),

    xml_attribute: ($) => seq($.identifier, '=', $.string),

    anonymous_function: ($) =>
    prec(
      PREC.PRIMARY,
      seq(
        'function',
        optional(field('name', $.identifier)),
        '(',
        field('parameters', optional($.function_parameters)),
        ')',
        optional(field('return_type', $.type_hint)),
        field('body', $.statement_block)
      )
    ),

    call_expression: ($) =>
    prec(
      PREC.PRIMARY,
      seq(
        field('function', $.expression),
        '(',
        field('parameters', optional(sep1($.expression, ','))),
        ')'
      )
    ),

    assignment_expression: ($) =>
    prec(
      PREC.ASSIGNEMENT,
      seq(
        field(
          'left',
          choice($.member_expression, $.subscript_expression, $.identifier)
        ),
        '=',
        field('right', $.expression)
      )
    ),

    augmented_assignment_expression: ($) =>
    prec(
      PREC.ASSIGNEMENT,
      seq(
        field(
          'left',
          choice($.member_expression, $.subscript_expression, $.identifier)
        ),
        field(
          'operator',
          choice(
            '*=',
            '/=',
            '%=',
            '+=',
            '-=',
            '<<=',
            '>>=',
            '>>>=',
            '&=',
            '^=',
            '|='
          )
        ),
        field('right', $.expression)
      )
    ),

    unary_expression: ($) =>
    prec(
      PREC.UNARY,
      seq(
        field(
          'operator',
          choice('+', '-', '~', '!', 'delete', 'typeof', 'void')
        ),
        field('argument', $.expression)
      )
    ),

    binary_expression: ($) => choice(
      ...[
        ['*', PREC.MULTIPLICATIVE],
        ['/', PREC.MULTIPLICATIVE],
        ['%', PREC.MULTIPLICATIVE],
        ['+', PREC.ADDITIVE],
        ['-', PREC.ADDITIVE],
        ['<<', PREC.BITWISE_SHIFT],
        ['>>', PREC.BITWISE_SHIFT],
        ['>>>', PREC.BITWISE_SHIFT],
        ['<', PREC.RELATIONAL],
        ['>', PREC.RELATIONAL],
        ['<=', PREC.RELATIONAL],
        ['>=', PREC.RELATIONAL],
        // ['as', PREC.RELATIONAL],
        ['in', PREC.RELATIONAL],
        ['instanceof', PREC.RELATIONAL],
        ['is', PREC.RELATIONAL],
        ['==', PREC.EQUALITY],
        ['!=', PREC.EQUALITY],
        ['===', PREC.EQUALITY],
        ['!==', PREC.EQUALITY],
        ['&', PREC.BITWISE_AND],
        ['^', PREC.BITWISE_XOR],
        ['|', PREC.BITWISE_OR],
        ['&&', PREC.LOGICAL_AND],
        ['||', PREC.LOGICAL_OR],
      ].map(([op, pre]) =>
        prec.left(pre, seq($.expression, op, $.expression))
      )
    ),

    cast_expression: ($) => prec.left(
      PREC.RELATIONAL,
      seq($.expression, 'as', $._data_type)),

    ternary_expression: ($) => prec.right(
      PREC.CONDITIONAL,
      seq(
        field('condition', $.expression),
        '?',
        field('iftrue', $.expression),
        ':',
        field('iffalse', $.expression)
      )
    ),

    update_expression: ($) => choice(
      prec(
        PREC.POSTFIX,
        seq(
          field('argument', $.expression),
          field('operator', choice('++', '--'))
        )
      ),
      prec(
        PREC.UNARY,
        seq(
          field('operator', choice('++', '--')),
          field('argument', $.expression)
        )
      )
    ),
    new_expression: ($) => prec(PREC.PRIMARY, seq('new', $.primary_expression)),

    // Data types

    _data_type: ($) => prec.right(
      choice(
        $.any_type,
        $.identifier,
        $.generic_data_type,
        $.scoped_data_type
      )
    ),

    any_type: ($) => '*',

    generic_data_type: ($) =>
    seq(
      $.identifier,
      '.<',
      field('type_parameters', sep1($._data_type, ',')),
      '>'
    ),

    scoped_data_type: ($) => seq($.identifier, '.', $._data_type),

    type_hint: ($) => seq(':', field('type', $._data_type)),

    // Primitive

    true: ($) => 'true',
    false: ($) => 'false',
    undefined: ($) => 'undefined',
    null: ($) => 'null',

    // from https://github.com/tree-sitter/tree-sitter-javascript/blob/master/grammar.js
    number: ($) => {
      const hex_literal = seq(choice('0x', '0X'), /[\da-fA-F](_*[\da-fA-F])*/);

      const decimal_digits = /\d(_*\d)*/;
      const signed_integer = seq(optional(choice('-', '+')), decimal_digits);
      const exponent_part = seq(choice('e', 'E'), signed_integer);

      const binary_literal = seq(choice('0b', '0B'), /[0-1](_*[0-1])*/);

      const octal_literal = seq(choice('0o', '0O'), /[0-7](_*[0-7])*/);

      const bigint_literal = seq(
        choice(hex_literal, binary_literal, octal_literal, decimal_digits),
        'n'
      );

      const decimal_integer_literal = choice(
        '0',
        seq(optional('0'), /[1-9]/, optional(seq(repeat('_'), decimal_digits)))
      );

      const decimal_literal = choice(
        seq(
          decimal_integer_literal,
          '.',
          optional(decimal_digits),
          optional(exponent_part)
        ),
        seq('.', decimal_digits, optional(exponent_part)),
        seq(decimal_integer_literal, exponent_part),
        seq(decimal_digits)
      );

      // Numeric constants
      // https://help.adobe.com/en_US/FlashPlatform/reference/actionscript/3/package-detail.html
      const nan = 'NaN';
      const pInfinity = 'Infinity';
      const mInfinity = '-Infinity';

      return token(
        choice(
          hex_literal,
          decimal_literal,
          binary_literal,
          octal_literal,
          bigint_literal,
          nan,
          pInfinity,
          mInfinity
        )
      );
    },

    // number: ($) =>
    //   choice(
    //     $._nan,
    //     $._pInfinity,
    //     $._mInfinity,
    //     $._hex_literal,
    //     $._decimal_literal
    //   ),

    // _hex_literal: ($) => seq(choice('0x', '0X'), /[0-9a-fA-F][0-9a-fA-F_]*/),
    // // _hex_literal: ($) => seq(choice('0x', '0X'), sep1(/[0-9a-fA-F]+/, /_+/)),
    // // TODO: Octal should be 0777 like numbers
    // // _oct_literal: ($) => '',

    // _integer_literal: ($) => /[0-9][0-9_]*/,
    // _exponential_part: ($) =>
    //   seq(choice('e', 'E'), optional(choice('+', '-')), $._integer_literal),
    // _decimal_literal: ($) =>
    //   prec.right(
    //     seq(
    //       $._integer_literal,
    //       optional(seq('.', $._integer_literal)),
    //       optional($._exponential_part)
    //     )
    //   ),

    // String literals

    // From tree-sitter-javascript string literal
    // https://github.com/tree-sitter/tree-sitter-javascript/blob/master/grammar.js
    string: ($) =>
    choice(
      seq(
        '"',
        repeat(
          choice($._unescaped_double_string_fragment, $._escape_sequence)
        ),
        '"'
      ),
      seq(
        "'",
        repeat(
          choice($._unescaped_single_string_fragment, $._escape_sequence)
        ),
        "'"
      )
    ),

    _unescaped_double_string_fragment: ($) => token.immediate(prec(1, /[^"\\]+/)),

    _unescaped_single_string_fragment: ($) => token.immediate(prec(1, /[^'\\]+/)),

    _escape_sequence: ($) => token.immediate(
      seq(
        '\\',
        choice(
          /[^xu0-7]/,
          /[0-7]{1,3}/,
          /x[0-9a-fA-F]{2}/,
          /u[0-9a-fA-F]{4}/,
          /u\{[0-9a-fA-F]+\}/,
        )
      )
    ),

    regex: ($) => seq(
      '/',
      field('pattern', $.regex_pattern),
      token.immediate('/'),
      optional(field('flags', $.regex_flags))
    ),

    regex_pattern: ($) =>
    token.immediate(
      prec(
        -1,
        repeat1(
          choice(
            seq(
              '[',
              repeat(
                choice(
                  seq('\\', /./), // escaped character
                  /[^\]\n\\]/ // any character besides ']' or '\n'
                )
              ),
              ']'
            ), // square-bracket-delimited character class
            seq('\\', /./), // escaped character
            /[^/\\\[\n]/ // any character besides '[', '\', '/', '\n'
          )
        )
      )
    ),

    regex_flags: ($) => token.immediate(/[a-z]+/),

    // symbol "#", "$" "§" because they can show up in decompiled code
    // TODO: "@" symbol is the attribute identifier operator, used to access attribute
    // in XML data
    identifier: ($) => /[\p{L}_$#§@][\p{L}\p{Nd}_$#§@]*/,

    // Comments

    comment: ($) => choice($.line_comment, $.block_comment),
    line_comment: ($) => token(seq('//', /[^\n]*/)),
    block_comment: ($) => token(seq('/*', /[^*]*\*+([^/*][^*]*\*+)*/, '/')),
  },
});

function sep1(rule, separator) {
  return seq(rule, repeat(seq(separator, rule)));
}

function caseInsensitive(keyword) {
  return new RegExp(
    keyword
      .split('')
      .map((letter) => `[${letter}${letter.toUpperCase()}]`)
      .join('')
  );
}
