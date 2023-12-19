// Partial
type Person_O = {
  name: string;
  age: number;
};

// 直接使用初始化所有参数都是必填
let tom2: Person_O = {
  name: "tom",
  age: 20,
};

// 使用Partial将Person中所有的参数变为非必填
type PartialPerson = Partial<Person_O>;

let partialPerson: PartialPerson = {
  name: "tom",
};

// Record
type petsGroup = "dog" | "cat" | "fish";

type numOrStr = number | string;

type IPets = Record<petsGroup, numOrStr>;

// type IPets = {
//     dog: numOrStr;
//     cat: numOrStr;
//     fish: numOrStr;
// }

// Pick
interface B {
  id: number;
  name: string;
  age: number;
}

type PickB = Pick<B, "id" | "name">;

// type PickB = {
//     id: number;
//     name: string;
// }

// Exclude
// 例子1
type T = {
  name: string;
  age: number;
};

type U = {
  name: string;
};

type IType = Exclude<keyof T, keyof U>;
// type IType = "age"

type T0 = Exclude<"a" | "b" | "c", "a" | "b">;
// type T0 = "c"

type T1 = Exclude<"a" | "b" | "c", "a" | "b" | "s">;
// type T1 = "c"

// Extract
type T2 = Extract<"a" | "b" | "c", "a" | "f">;
// type T0 = "a"

type T3 = {
  name: string;
  age: number;
};

type U3 = {
  name: string;
};

type IType3 = Extract<keyof T3, keyof U3>;
// type IType = "name"

// ConstructorParameters
class People {
  name: string;
  age: number;

  constructor(name: string) {
    this.name = name;
  }
}

type IType_ConstructorParameters = ConstructorParameters<typeof People>;
// type IType = [name: string]
// 注意这里typeof操作是取类型的作用

// InstanceType
class People_InstanceType {
  name: string;
  age: number;

  constructor(name: string) {
    this.name = name;
  }
}

type IType_InstanceType = InstanceType<typeof People_InstanceType>;
// type IType = People_InstanceType
// 因为constructor默认返回this
// constructor People_InstanceType(name: string): People_InstanceType

// NonNullable
type example = NonNullable<string | number | undefined>;
// type example = string | number

// Parameters & ReturnType
type IFoo = (
  uname: string,
  uage: number
) => {
  name: string;
  age: number;
};

//参数类型
type Ibar = Parameters<IFoo>;
// type Ibar = [uname: string, uage: number]

type T_RT = ReturnType<IFoo>;
// type T_RT = {
//     name: string;
//     age: number;
// }

// readonly & ReadonlyArray
interface Person_readonly {
  readonly id: number;
}
const data: Person_readonly = {
  id: 456,
};

//   data.id = 789;
// 无法分配到 "id" ，因为它是只读属性。ts(2540)

const arr: number[] = [1, 2, 3, 4];

let ro: ReadonlyArray<number> = arr;

//   ro.push(444);
// 类型“readonly number[]”上不存在属性“push”。ts(2339)
