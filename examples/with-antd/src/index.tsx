import React from 'react';
import ReactDOM from 'react-dom/client';
import { createHashRouter, RouterProvider } from 'react-router-dom';
import { Layout } from './layout';
import { Home } from './home';
import { Todos } from './todos';
import { ReactQuery } from './react-query';

const router = createHashRouter([
  {
    path: '/',
    element: <Layout />,
    children: [
      {
        path: '/todos',
        element: <Todos />,
      },
      {
        path: '/react-query',
        element: <ReactQuery />,
      },
      {
        path: '/',
        element: <Home />,
      },
    ],
  },
]);

function App() {
  return (
    <div>
      <RouterProvider router={router} />
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')).render(<App />);
