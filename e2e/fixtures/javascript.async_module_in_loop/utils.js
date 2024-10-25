import {config} from "./component"

export function listKeys() {
    if(config){
        
    return Object.keys(config)
    }
    return ["oops"]
}