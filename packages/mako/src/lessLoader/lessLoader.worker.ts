import workerpool from 'workerpool';
import { render } from './render';

workerpool.worker({ render });
