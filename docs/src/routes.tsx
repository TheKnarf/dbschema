import App, { Home } from './app.tsx';
import { Outlet } from 'react-router';
import DocsIndex from './docs/index.mdx';
import Installation from './docs/installation.mdx';
import GettingStarted from './docs/getting-started.mdx';
import Variable from './docs/variable.mdx';
import Locals from './docs/locals.mdx';
import SchemaDoc from './docs/schema.mdx';
import EnumDoc from './docs/enum.mdx';
import TableDoc from './docs/table.mdx';
import ViewDoc from './docs/view.mdx';
import MaterializedDoc from './docs/materialized.mdx';
import FunctionDoc from './docs/function.mdx';
import TriggerDoc from './docs/trigger.mdx';
import ExtensionDoc from './docs/extension.mdx';
import PolicyDoc from './docs/policy.mdx';
import ModuleDoc from './docs/module.mdx';
import OutputDoc from './docs/output.mdx';
import TestDoc from './docs/test.mdx';

const DocsLayout = () => <Outlet />;

const routes = [
  {
    path: '/',
    element: <App />,
    children: [
      { index: true, element: <Home /> },
      {
        path: 'docs',
        element: <DocsLayout />,
        children: [
          { index: true, element: <DocsIndex /> },
          { path: 'installation', element: <Installation /> },
          { path: 'getting-started', element: <GettingStarted /> },
          { path: 'variable', element: <Variable /> },
          { path: 'locals', element: <Locals /> },
          { path: 'schema', element: <SchemaDoc /> },
          { path: 'enum', element: <EnumDoc /> },
          { path: 'table', element: <TableDoc /> },
          { path: 'view', element: <ViewDoc /> },
          { path: 'materialized', element: <MaterializedDoc /> },
          { path: 'function', element: <FunctionDoc /> },
          { path: 'trigger', element: <TriggerDoc /> },
          { path: 'extension', element: <ExtensionDoc /> },
          { path: 'policy', element: <PolicyDoc /> },
          { path: 'module', element: <ModuleDoc /> },
          { path: 'output', element: <OutputDoc /> },
          { path: 'test', element: <TestDoc /> },
        ],
      },
    ],
  },
];

export default routes;
