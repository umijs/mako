// These operators may have side effects
let keep;
+keep;
-keep;
~keep;
// in swc, 'delete' cannot be called on an identifier in strict mode
// delete keep;
++keep;
--keep;
keep++;
keep--;

// These operators never have side effects
let REMOVE;
!REMOVE;
void REMOVE;
