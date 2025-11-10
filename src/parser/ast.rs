pub use crate::types::Spanned;

pub type StmtS = Spanned<Stmt>;
pub type ExprS = Spanned<Expr>;

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Bool(bool),
    Int(i64),
    String(String),
    Float(f64),
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Not,
    Negate,
    Pos,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    FloorDivide,
    Modulo,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Literal),
    Variable(String),
    Unary {
        op: UnaryOp,
        expr: Box<ExprS>,
    },
    Binary {
        op: BinaryOp,
        left: Box<ExprS>,
        right: Box<ExprS>,
    },
    Call {
        func_name: Box<ExprS>,
        args: Vec<ExprS>,
    },
    Attribute {
        object: Box<ExprS>,
        attr: String,
    },
    List(Vec<ExprS>),
    Dict(Vec<(ExprS, ExprS)>),
    Index {
        object: Box<ExprS>,
        index: Box<ExprS>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    If {
        condition: ExprS,
        then_block: Vec<StmtS>,
        elif_blocks: Vec<(ExprS, Vec<StmtS>)>,
        else_block: Option<Vec<StmtS>>,
    },
    While {
        condition: ExprS,
        body: Vec<StmtS>,
    },
    For {
        var: String,
        iterable: ExprS,
        body: Vec<StmtS>,
    },
    Def {
        name: String,
        params: Vec<String>,
        body: Vec<StmtS>,
    },
    Return(ExprS),
    Assign {
        target: ExprS,
        value: ExprS,
    },
    Class {
        name: String,
        methods: Vec<MethodDef>,
        attributes: Vec<(String, ExprS)>,
    },
    Expr(ExprS),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MethodDef {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<StmtS>,
}
