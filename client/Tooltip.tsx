import cls from 'classnames';
import { FC, useEffect, useRef, useState } from 'react';
import s from './Tooltip.module.css';

interface TooltipProps {
  visible: boolean;
  content: string;
}
const Tooltip: FC<TooltipProps> = ({ visible, content }) => {
  const [position, setPosition] = useState({ left: 0, top: 0 });

  const mouseCoords = useRef({ x: 0, y: 0 });
  const nodeRef = useRef(null);

  const marginX = 10;
  const marginY = 30;
  const handleMouseMove = (event) => {
    mouseCoords.current = { x: event.pageX, y: event.pageY };
    if (visible) {
      updatePosition();
    }
  };
  useEffect(() => {
    document.addEventListener('mousemove', handleMouseMove, true);

    return () => {
      document.removeEventListener('mousemove', handleMouseMove, true);
    };
  }, [visible]); // Add visible as a dependency

  // Only update if visible changes to true
  const shouldComponentUpdate = (nextProps) => {
    return visible || nextProps.visible;
  };

  const updatePosition = () => {
    if (!visible) return;

    const pos = {
      left: mouseCoords.current.x + marginX,
      top: mouseCoords.current.y + marginY,
    };

    const boundingRect = nodeRef.current.getBoundingClientRect();
    if (pos.left + boundingRect.width > window.innerWidth) {
      // Shifting horizontally
      pos.left = window.innerWidth - boundingRect.width;
    }
    if (pos.top + boundingRect.height > window.innerHeight) {
      // Flipping vertically
      pos.top = mouseCoords.current.y - marginY - boundingRect.height;
    }

    setPosition(pos);
  };

  const className = cls({
    [s.container]: true,
    [s.hidden]: !visible,
  });

  return (
    <div
      ref={nodeRef}
      className={className}
      style={{ left: position.left, top: position.top }}
    >
      {content}
    </div>
  );
};

export default Tooltip;
