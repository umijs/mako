import { listKeys } from "./utils"

import { named } from "./async"

export const config = {
    key: "value"
}

export function displayConfig() {
    return listKeys()
}