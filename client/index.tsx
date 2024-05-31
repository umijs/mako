import { filesize } from 'filesize';
import React, { useEffect, useRef, useState } from 'react';
import ReactDOM from 'react-dom/client';

import FoamTree from '@carrotsearch/foamtree';
import Tooltip from './Tooltip';
import s from './Tooltip.module.css';
import Folder from './classUtils/Folder';

function App() {
  const chartRef = useRef<HTMLDivElement>(null);
  const [visible, setVisible] = useState(false);
  // toolTip展示使用
  const [tooltipContent, setToolTipContent] = useState('');
  const createModulesTree = (modules) => {
    const root = new Folder('.');
    modules.forEach((module) => root.addModule(module));
    root.mergeNestedFolders();
    return root;
  };
  const format = (modules: any[]) => {
    const root = createModulesTree(modules);
    const res = {
      'index.js': {
        tree: root,
      },
    };

    const data = Object.entries(res).map(([filename, asset]) => {
      return {
        label: filename,
        isAsset: true,
        // Not using `asset.size` here provided by Webpack because it can be very confusing when `UglifyJsPlugin` is used.
        // In this case all module sizes from stats file will represent unminified module sizes, but `asset.size` will
        // be the size of minified bundle.
        // Using `asset.size` only if current asset doesn't contain any modules (resulting size equals 0)
        statSize: asset.tree.size || asset.size,
        parsedSize: asset?.parsedSize,
        gzipSize: asset?.gzipSize,
        groups: Object.values(asset.tree?.children).map((i) => i.toChartData()),
      };
    });
    return data;
    // if(){}
    if (data) {
      new FoamTree({
        element: chartRef.current,
        layout: 'squarified',
        stacking: 'flattened',
        pixelRatio: window.devicePixelRatio || 1,
        maxGroups: Infinity,
        maxGroupLevelsDrawn: Infinity,
        maxGroupLabelLevelsDrawn: Infinity,
        maxGroupLevelsAttached: Infinity,
        wireframeLabelDrawing: 'always',
        groupMinDiameter: 0,
        groupLabelVerticalPadding: 0.2,
        rolloutDuration: 0,
        pullbackDuration: 0,
        fadeDuration: 0,
        groupExposureZoomMargin: 0.2,
        zoomMouseWheelDuration: 300,
        openCloseDuration: 200,
        dataObject: data,
      });
    }
  };
  const filterModulesForSize = (modules, sizeProp) => {
    return modules.reduce((filteredModules, module) => {
      if (module[sizeProp]) {
        if (module.groups) {
          const showContent = !module.concatenated || false;

          module = {
            ...module,
            groups: showContent
              ? filterModulesForSize(module.groups, sizeProp)
              : null,
          };
        }

        module.weight = module[sizeProp];
        filteredModules.push(module);
      }

      return filteredModules;
    }, []);
  };
  const renderModuleSize = (module, sizeType) => {
    const sizeProp = `${sizeType}Size`;
    const size = module[sizeProp];
    const sizeLabel = 'Size';
    return typeof size === 'number' ? (
      <div className={s.activeSize}>
        {sizeLabel} size: <strong>{filesize(size)}</strong>
      </div>
    ) : null;
  };
  // 格式化module数据到 toolTip 中
  const getTooltipContent = (module) => {
    if (!module) return null;

    return (
      <div>
        <div>
          <strong>{module.label}</strong>
        </div>
        <br />

        {renderModuleSize(module, 'stat')}
        {module.path && (
          <div>
            Path: <strong>{module.path}</strong>
          </div>
        )}
        {module.isAsset && (
          <div>
            <br />
            <strong>
              <em>Right-click to view options related to this chunk</em>
            </strong>
          </div>
        )}
      </div>
    );
  };
  useEffect(() => {
    window.addEventListener('load', () => {
      const chartData = window.chartData;
      const formatData = format(chartData?.modules || []);
      const resData = filterModulesForSize(formatData, 'statSize');
      new FoamTree({
        element: chartRef.current,
        layout: 'squarified',
        stacking: 'flattened',
        pixelRatio: window.devicePixelRatio || 1,
        maxGroups: Infinity,
        maxGroupLevelsDrawn: Infinity,
        maxGroupLabelLevelsDrawn: Infinity,
        maxGroupLevelsAttached: Infinity,
        wireframeLabelDrawing: 'always',
        groupMinDiameter: 0,
        groupLabelVerticalPadding: 0.2,
        rolloutDuration: 0,
        pullbackDuration: 0,
        fadeDuration: 0,
        groupExposureZoomMargin: 0.2,
        zoomMouseWheelDuration: 300,
        openCloseDuration: 200,
        dataObject: { groups: resData },
        titleBarDecorator(opts, props, vars) {
          vars.titleBarShown = false;
        },
        onGroupHover(event: { group: any }) {
          // 判断是否移动到组中
          const { group } = event;
          // 显示 tooltip
          if (group) {
            setVisible(true);
            setToolTipContent(getTooltipContent(group));
          } else {
            setVisible(false);
          }
        },
      });
    });
  }, []);

  return (
    <>
      <div style={{ width: '100vw', height: '100vh' }} ref={chartRef}></div>
      <Tooltip visible={visible} content={tooltipContent} />
    </>
  );
}

ReactDOM.createRoot(document.getElementById('root')!).render(<App />);
