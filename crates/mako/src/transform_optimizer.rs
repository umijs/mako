use swc_common::DUMMY_SP;
use swc_ecma_ast::{op, BinExpr, BinaryOp, Bool, EmptyStmt, Expr, IfStmt, Lit, Stmt};
use swc_ecma_visit::{VisitMut, VisitMutWith};

pub struct Optimizer;

enum OptimizeIfType {
    LitCompare(Lit, Lit, BinaryOp),
    Lit(Bool),
    Other,
}

impl VisitMut for Optimizer {
    fn visit_mut_stmt(&mut self, stmt: &mut Stmt) {
        // 优化 if 语句分支
        self.optimize_if(stmt);

        stmt.visit_mut_children_with(self);
    }
}

impl Optimizer {
    fn optimize_if(&self, stmt: &mut Stmt) {
        if let Stmt::If(ref if_stmt) = stmt.clone() {
            let optimize = |stmt: &mut Stmt, value: bool| {
                if value {
                    // 真值则舍弃 else 分支
                    *stmt = *if_stmt.cons.clone();
                } else {
                    // 假值则使用 else 分支或舍弃整个 if 语句（else 不存在的情况下）
                    match &if_stmt.alt {
                        Some(alt) => {
                            *stmt = *alt.clone();
                            // 针对 else if 的情况再处理一次
                            self.optimize_if(stmt);
                        }
                        None => *stmt = Stmt::Empty(EmptyStmt { span: DUMMY_SP }),
                    }
                }
            };

            match self.try_optimize_if(if_stmt) {
                // 计算常量比较结果
                OptimizeIfType::LitCompare(left, right, op) => {
                    if let Some(value) = self.compare_lits(left, right, op) {
                        optimize(stmt, value);
                    }
                }
                OptimizeIfType::Lit(bool) => {
                    optimize(stmt, bool.value);
                }
                OptimizeIfType::Other => {}
            }
        }
    }

    fn try_optimize_if(&self, stmt: &IfStmt) -> OptimizeIfType {
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
        if let Expr::Lit(Lit::Bool(bool)) = &*stmt.test {
            return OptimizeIfType::Lit(*bool);
        }
        OptimizeIfType::Other
    }

    fn compare_lits(&self, left: Lit, right: Lit, op: BinaryOp) -> Option<bool> {
        // 比较简单字面量的值（此处不会处理隐式转换）
        let compared = match (left, right) {
            (Lit::Str(str), Lit::Str(str2)) => Some(str.value == str2.value),
            (Lit::Num(num), Lit::Num(num2)) => Some(num.value == num2.value),
            (Lit::Bool(bool), Lit::Bool(bool2)) => Some(bool.value == bool2.value),
            (Lit::Null(_), Lit::Null(_)) => Some(true),
            _ => None,
        };

        compared.and_then(|is_equal| match op {
            op!("==") | op!("===") => Some(is_equal),
            op!("!=") | op!("!==") => Some(!is_equal),
            _ => None,
        })
    }
}
