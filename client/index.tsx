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

  // ref 用于保存Treemap实例
  const treeMapRef = useRef(null);
  const [chartData, setChartData] = useState('');

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
  // 格式化 module 数据到 toolTip 中
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
  const getFoamTreeData = (chartData: any) => {
    const formatData = format(chartData?.chunkModules || []);
    const resData = filterModulesForSize(formatData, 'statSize');
    return { groups: resData };
  };
  const createFoamTree = (chartData: any) => {
    const formatData = format(chartData?.chunkModules || []);
    const resData = filterModulesForSize(formatData, 'statSize');
    debugger;
    return new FoamTree({
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
      onMouseLeave() {
        setVisible(false);
      },
      onGroupHover(event: { group: any }) {
        // 判断是否移动到组中
        const { group } = event;
        // 表示已经移出，需要隐藏 toolTip
        if (group?.attribution) {
          setVisible(false);
          return;
        }
        console.log('group==', group);
        // 显示 tooltip
        if (group) {
          setVisible(true);
          setToolTipContent(getTooltipContent(group));
        } else {
          setVisible(false);
        }
      },
    });
  };
  const resize = () => {
    if (treeMapRef.current) {
      treeMapRef.current.resize();
    }
  };
  useEffect(() => {
    window.addEventListener('load', () => {
      setChartData(window?.chartData);
      // 如果开启了热更新,那么启动 websocket 服务。
      if (window?.hmrWatch) {
        const socket = new WebSocket('ws://localhost:3000/__/sendStatsData');

        socket.addEventListener('message', (rawMessage) => {
          const msg = JSON.parse(rawMessage.data);
          console.log('msg==', msg);
          setChartData(msg);
        });
      }
    });
    window.addEventListener('resize', resize);
    return () => {
      window.removeEventListener('resize', resize);
    };
  }, []);
  useEffect(() => {
    if (!chartData) {
      console.warn('数据未初始化!!');
      return;
    }
    // 如果已经实例化并且 chartData 发生改变，那么就重新设置值。
    if (treeMapRef.current) {
      debugger;
      treeMapRef.current.set({
        dataObject: getFoamTreeData(chartData),
      });
      treeMapRef.current.update();
      return;
    }
    treeMapRef.current = createFoamTree(chartData);
    return () => {
      if (treeMapRef.current) {
        treeMapRef.current.dispose();
        treeMapRef.current = null;
      }
    };
  }, [chartData]);
  return (
    <>
      <div style={{ width: '100vw', height: '100vh' }} ref={chartRef}></div>
      <Tooltip visible={visible} content={tooltipContent} />
    </>
  );
}

ReactDOM.createRoot(document.getElementById('root')!).render(<App />);
