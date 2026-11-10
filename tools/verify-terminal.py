"""Check actual QEMU pixels and serial bytes from the terminal probe."""
import argparse
from pathlib import Path
from PIL import Image

p=argparse.ArgumentParser()
p.add_argument('--cpus',type=int,default=1)
a=p.parse_args()
root=Path(__file__).resolve().parent.parent/'build'
covered=Image.open(root/f'terminal-covered-{a.cpus}.ppm').convert('RGB')
restored=Image.open(root/f'terminal-qemu-{a.cpus}.ppm').convert('RGB')
scroll=Image.open(root/f'terminal-scroll-{a.cpus}.ppm').convert('RGB')
def pixels(image):
    data=image.tobytes()
    return list(zip(data[0::3],data[1::3],data[2::3]))
assert covered.size==restored.size
assert all(pixel==(160,0,160) for pixel in pixels(covered.crop((0,0,420,60)))), 'TTY output overwrote the window'
top=pixels(restored.crop((0,0,420,60)))
assert (160,0,160) not in top, 'Window pixels survived destroy'
assert sum(max(pixel)>0 for pixel in top)>100, 'Covered terminal text was not restored'
assert any(r>80 and g==0 and b==0 for r,g,b in top), 'Red foreground missing'
assert any(g>80 and r==0 and b==0 for r,g,b in top), 'Green foreground missing'
assert any(b>80 and r==0 and g==0 for r,g,b in top), 'Blue background/foreground missing'
assert any(g>80 and r==0 and b==0 for r,g,b in pixels(scroll.crop((0,0,400,24)))), 'Scrolling cleared the whole display'
serial=(root/f'ctrl-c-qemu-{a.cpus}.log').read_bytes()
assert b'raw line A\nB\r\n' in serial, 'OPOST-disabled output was transformed'
assert b'UTF-8 split: \xc3\xa9 / invalid: \xff\r\n' in serial, 'TTY did not preserve split/invalid bytes'
assert b'[terminal] PASS' in serial
assert b'[terminal-scroll] PASS' in serial
covered.save(root/f'terminal-covered-{a.cpus}.png')
restored.save(root/f'terminal-qemu-{a.cpus}.png')
scroll.save(root/f'terminal-scroll-{a.cpus}.png')
restored.crop((0,0,800,360)).save(root/f'terminal-detail-{a.cpus}.png')
print(f'PASS: {a.cpus} CPU terminal pixels, window layering/restoration, scrolling, UTF-8 and raw output bytes')
