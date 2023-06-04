use swc_common::DUMMY_SP;
use swc_ecma_ast::{op, BinExpr, BinaryOp, EmptyStmt, Expr, IfStmt, Lit, Stmt};
use swc_ecma_visit::{VisitMut, VisitMutWith};

pub struct Optimizer;

enum OptimizeIfType {
    LitCompare(Lit, Lit, BinaryOp),
    Other,
}

impl VisitMut for Optimizer {
    fn visit_mut_stmt(&mut self, stmt: &mut Stmt) {
        // 优化 if 语句分支
        if let Stmt::If(if_stmt) = stmt {
            match self.try_optimize_if(if_stmt) {
                // 计算常量比较结果
                OptimizeIfType::LitCompare(left, right, op) => {
                    if let Some(value) = self.compare_lits(left, right, op) {
                        if value {
                            // 真值则舍弃 else 分支
                            *stmt = *if_stmt.cons.clone();
                        } else {
                            // 假值则使用 else 分支或舍弃整个 if 语句（else 不存在的情况下）
                            match &if_stmt.alt {
                                Some(alt) => *stmt = *alt.clone(),
                                None => *stmt = Stmt::Empty(EmptyStmt { span: DUMMY_SP }),
                            }
                        }
                    }
                }
                OptimizeIfType::Other => {}
            }
        }
        stmt.visit_mut_children_with(self);
    }
}

impl Optimizer {
    fn try_optimize_if(&self, stmt: &mut IfStmt) -> OptimizeIfType {
        if let Expr::Bin(BinExpr {
            left, right, op, ..
        }) = &*stmt.test
        {
            if left.is_lit() && right.is_lit() {
                return OptimizeIfType::LitCompare(
                    left.as_lit().unwrap().clone(),
                    right.as_lit().unwrap().clone(),
                    *op,
                );
            }
        }
        OptimizeIfType::Other
    }

    fn compare_lits(&self, left: Lit, right: Lit, op: BinaryOp) -> Option<bool> {
        // 比较简单字面量的值（此处不会处理隐式转换）
        let is_equal = match left {
            Lit::Str(str) => match right {
                Lit::Str(str2) => str.value == str2.value,
                _ => false,
            },
            Lit::Num(num) => match right {
                Lit::Num(num2) => num.value == num2.value,
                _ => false,
            },
            Lit::Bool(bool) => match right {
                Lit::Bool(bool2) => bool.value == bool2.value,
                _ => false,
            },
            _ => false,
        };

        match op {
            op!("==") | op!("===") => Some(is_equal),
            op!("!=") | op!("!==") => Some(!is_equal),
            _ => None,
        }
    }
}
