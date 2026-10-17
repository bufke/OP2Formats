import subprocess,re,sys,json,os
sys.path.insert(0,sys.argv[1]); import pe
# (weapon code, [guardpost, tiger, panther, lynx] addresses) from TethysAPI MapObjectType.h order
src=open(os.environ['TETHYS_MAPOBJECTTYPE_H']).read()
cls=None; out={}
lines=src.split('\n')
for i,l in enumerate(lines):
    m=re.match(r'class (\w+) : public MapEntityType',l)
    if m: cls=m.group(1)
    m=re.search(r'virtual void Select(\w+)Graphic|void Select(\w+)Graphic\(MapObject',l)
    if m and cls and 'override' in l:
        kind=m.group(1) or m.group(2)
        addr=int(re.search(r'0x([0-9A-F]+)',lines[i+1]).group(1),16)
        out.setdefault(cls,{})[kind]=addr
res={}
for cls,fns in out.items():
    for kind,addr in fns.items():
        dis=subprocess.run(['objdump','-d','-M','intel','--start-address=%#x'%addr,'--stop-address=%#x'%(addr+0x90),pe.EXE],capture_output=True,text=True).stdout
        body=[]
        for l in dis.splitlines()[7:]:
            body.append(l.split('\t')[-1].strip())
            if body[-1].startswith('ret'): break
        tabs=[int(x,16) for x in re.findall(r'\[e[a-d]x\+0x(4e[0-9a-f]{4})\]',' '.join(body))]
        imms=[int(x,16) for x in re.findall(r'mov\s+DWORD PTR ds:0x4ea7b8,0x([0-9a-f]+)',' '.join(body))]
        res.setdefault(cls,{})[kind]={'addr':hex(addr),'tables':[[pe.u32(t+4*k)-101 for k in range(16)] for t in tabs],'imm':[v-101 for v in imms],'n':len(body)}
json.dump(res,open(sys.argv[2],'w'),indent=1)
for c,f in res.items():
    for k,v in f.items(): print(c,k,v['addr'],'imm',v['imm'],'tables',[t[:4] for t in v['tables']],v['n'])
