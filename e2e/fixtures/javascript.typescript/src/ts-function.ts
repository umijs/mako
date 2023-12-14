// 1.函数声明
// 输入多余的（或者少于要求的）参数，是不被允许的
sum(1, 2, 3);
// error TS2346: Supplied parameters do not match any signature of call target.
sum(1);
// error TS2346: Supplied parameters do not match any signature of call target.

// 入参不强制限定
function sum(x?: number, y?: number, ...args: number[]): number {
  return (x || 0) + (y || 0) + args.reduce((pre, cur) => pre + cur, 0);
}
// TypeScript 会将添加了默认值的参数识别为可选参数
// function sum(x: number = 0, y: number = 0, ...args: number[]): number {
//     return (x || 0) + (y || 0) + args.reduce((pre, cur) => pre + cur, 0);
// }
// 注意：可选参数必须接在必需参数后面
// error TS1016: A required parameter cannot follow an optional parameter.

sum(1);
sum(1, 2);
sum(1, 2, 3);

// 参数为对象类型
// function copy(source: object): object {
//     return { ...source }
// }
// function copy(source: { name: string; age: number }): object {
//     return { ...source }
// }
// 注意一下解构形式，依然要对整个参数添加类型，而不是解构后的属性
function copy({ name, age }: { name: string; age: number }): object {
  return { name, age };
}
// type IPerson = { name: string; age: number };
// function copy({ name, age }: IPerson): object {
//     return { name, age }
// }

// 2.函数表达式
// 等号右侧的匿名函数进行了类型定义，而等号左边的mySum，是通过赋值操作进行类型推论而推断出来的
let mySum = function (x: number, y: number): number {
  return x + y;
};

// 手动添加类型
let mySum2: (x: number, y: number) => number = function (
  x: number,
  y: number
): number {
  return x + y;
};
// 注意区分 TypeScript 中的=>和 ES6 中的=>
// type MySumFunc = (x: number, y: number) => number;
// let mySum: MySumFunc = function (x: number, y: number): number {
//     return x + y;
// };

// 3.箭头函数
// 类型推论
const mySum_a = (x: number, y: number): number => {
  return x + y;
};

// 完整定义
const mySum_a2: (x: number, y: number) => number = (
  x: number,
  y: number
): number => {
  return x + y;
};

// 定义变量的类型
// const mySum: (x: number, y: number) => number ;
// const mySum: (x: number, y: number) => number = (x, y) => {
//     return x + y;
// }
// // 常见的react function component
// const MyDiv: React.FC = (props) => <div {...props}/>
// // 源码
// type FC<P = {}> = FunctionComponent<P>;
// interface FunctionComponent<P = {}> {
//     (props: PropsWithChildren<P>, context?: any): ReactElement<any, any> | null;
//     propTypes?: WeakValidationMap<P>;
//     contextTypes?: ValidationMap<any>;
//     defaultProps?: Partial<P>;
//     displayName?: string;
// }

// 4.用接口定义函数的形状
interface SearchFunc {
  (source: string, subString: string): boolean;
}
// type SearchFunc = (source: string, subString: string) => boolean;

let mySearch: SearchFunc;
mySearch = function (source: string, subString: string) {
  return source.search(subString) !== -1;
};

// 5.重载
function reverse(x: number): number;
function reverse(x: string): string;
function reverse(x: number | string) {
  if (typeof x === "number") {
    return Number(x.toString().split("").reverse().join(""));
  } else if (typeof x === "string") {
    return x.split("").reverse().join("");
  }
}
