import struct
import os
EXE=os.environ["OP2_EXE"]
d=open(EXE,'rb').read()
secs=[(0x401000,0x400,0xcdbbd),(0x4cf000,0xce000,0xe754),(0x4de000,0xdc800,0x11e00)]
def off(va):
    for v,o,s in secs:
        if v<=va<v+s: return va-v+o
def u32(va): return struct.unpack_from('<I',d,off(va))[0]
def cstr(va):
    o=off(va); return d[o:d.index(b'\0',o)].decode('latin1')
def vtables():
    out=[]
    for va in range(0x4cf000,0x4cf000+0xe754-16,4):
        if u32(va+8)==0x444fe0 and u32(va+12)==0x445030:
            gcn=u32(va+4); o=off(gcn)
            if o and d[o]==0xb8 and d[o+5]==0xc3:
                name=cstr(struct.unpack_from('<I',d,o+1)[0])
                out.append((va,name,u32(va),gcn,u32(va+16)))
    return out
if __name__=='__main__':
    for va,name,c,g,i in vtables(): print(hex(va),name,hex(i))
