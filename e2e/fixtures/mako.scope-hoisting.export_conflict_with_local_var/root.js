import { models as rawModel } from "./inner";

export function makeModels() {
  const models = rawModel.map((model) => model + 2);

  return models;
}
