import { probe } from "./probe";
import * as ignored from "ignored-package";
export default { ...probe(), ignored: typeof ignored };
