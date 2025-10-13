pub use crate::types::Spanned;

pub type StmtS = Spanned<Stmt>;
pub type ExprS = Spanned<Expr>;

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Bool(bool),
    Int(i64),
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
        func_name: String,
        args: Vec<ExprS>,
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
    Def {
        name: String,
        params: Vec<String>,
        body: Vec<StmtS>,
    },
    Return(ExprS), // TODO: If 'None' is added to Literal, change to Option<ExprS>
    Assign {
        name: String,
        value: ExprS,
    },
    Expr(ExprS),
}
