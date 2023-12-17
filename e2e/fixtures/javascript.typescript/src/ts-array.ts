// T[]
let nums: number[] = [1,2,3,4,5];
let strs: string[] = ['1','2','3','4','5'];
let boos: boolean[] = [true,false];
let objs: object[] = [{},{key:"value"}];
let arrs: any[][] = [[],[1],["1"],[true],[{}],[[]]];
let funs: ((...args:any[])=>void)[] = [()=>{},(arg1)=>true];
// // 等同于
// let nums = [1,2,3,4,5]; // number[]
// let strs = ['1','2','3','4','5']; // string[]
// let boos = [true,false]; // boolean[]
// // 约等于
// let objs = [{},{key:"value"}]; // ({ key?: undefined; } | { key: string; })[]
// let arrs = [[],[1],["1"],[true],[{}],[[]]]; // (number[] | string[] | boolean[] | {}[] | never[][])[]
// let funs = [()=>{},(arg1:any)=>true]; // ((() => void) | ((arg1: any) => boolean))[]

// Array<T>
let g_nums: Array<number> = [1,2,3,4,5];
let g_strs: Array<string> = ['1','2','3','4','5'];
let g_boos: Array<boolean> = [true,false];
let g_objs: Array<object> = [{},{key:"value"}];
let g_arrs: Array<any[]> = [[],[1],["1"],[true],[{}],[[]]];
let g_funs: Array<(...args:any[])=>void> = [()=>{},(arg1)=>true];