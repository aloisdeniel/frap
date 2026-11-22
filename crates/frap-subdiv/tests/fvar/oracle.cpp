// SPDX-License-Identifier: GPL-3.0-or-later
// WP 8.11 development oracle: Blender 5.2.4's own OpenSubdiv (libosdCPU.dylib, OpenSubdiv
// 3.7.0) configured exactly as `intern/opensubdiv/internal/evaluator/evaluator_impl.cc` does,
// fed a descriptor written by `src/far/oracle_tests.rs` and dumping every stage (level sizes,
// face-varying stencils, patches, local points, appended stencils, refined values, corner UVs)
// in the format that test compares bit for bit.
//
// Build (OpenSubdiv 3.7.0 headers, e.g. a checkout of tag v3_7_0, and Blender's dylib):
//   L=~/Applications/Blender-5.2.app/Contents/Resources/lib
//   clang++ -std=c++14 -O1 -I<OpenSubdiv checkout> oracle.cpp -L$L -losdCPU -Wl,-rpath,$L -o oracle
// Run:
//   FRAP_OSD_ORACLE=$PWD/oracle FRAP_OSD_ORACLE_DIR=/tmp/out \
//     cargo test -p frap-subdiv --lib oracle -- --nocapture
#include <opensubdiv/far/topologyDescriptor.h>
#include <opensubdiv/far/stencilTableFactory.h>
#include <opensubdiv/far/patchTableFactory.h>
#include <opensubdiv/far/patchMap.h>
#include <opensubdiv/osd/cpuEvaluator.h>
#include <opensubdiv/osd/cpuPatchTable.h>
#include <opensubdiv/osd/cpuVertexBuffer.h>
#include <cstdio>
#include <cstring>
#include <vector>
#include <string>
using namespace OpenSubdiv;
using namespace OpenSubdiv::Far;
static float bf(unsigned b){float f; memcpy(&f,&b,4); return f;}
static unsigned fb(float f){unsigned b; memcpy(&b,&f,4); return b;}
static void dumpTable(FILE* o, const char* name, const StencilTable* t){
  if(!t){fprintf(o,"%s 0 0\n",name);return;}
  int n=t->GetNumStencils();
  fprintf(o,"%s %d %d\n",name,n,t->GetNumControlVertices());
  for(int i=0;i<n;i++){
    auto s=t->GetStencil(i);
    fprintf(o,"%d",s.GetSize());
    for(int j=0;j<s.GetSize();j++) fprintf(o," %d:%08x",s.GetVertexIndices()[j],fb(s.GetWeights()[j]));
    fprintf(o,"\n");
  }
}
int main(int argc,char**argv){
  FILE*fp=fopen(argv[1],"r"); FILE*o=fopen(argv[2],"w");
  int nv,nf; fscanf(fp,"%d %d",&nv,&nf);
  int vb,fl,adaptive,level; fscanf(fp,"%d %d %d %d",&vb,&fl,&adaptive,&level);
  std::vector<int> vpf(nf); int tot=0; for(auto&x:vpf){fscanf(fp,"%d",&x); tot+=x;}
  std::vector<int> fv(tot); for(auto&x:fv)fscanf(fp,"%d",&x);
  int nc; fscanf(fp,"%d",&nc);
  std::vector<int> cpairs(2*nc); std::vector<float> cw(nc);
  for(int i=0;i<nc;i++){unsigned b; fscanf(fp,"%d %d %x",&cpairs[2*i],&cpairs[2*i+1],&b); cw[i]=bf(b);}
  int nk; fscanf(fp,"%d",&nk);
  std::vector<int> kidx(nk); std::vector<float> kw(nk);
  for(int i=0;i<nk;i++){unsigned b; fscanf(fp,"%d %x",&kidx[i],&b); kw[i]=bf(b);}
  int nch; fscanf(fp,"%d",&nch);
  std::vector<std::vector<int>> chIdx(nch); std::vector<std::vector<float>> chVal(nch); std::vector<int> chN(nch);
  for(int c=0;c<nch;c++){ fscanf(fp,"%d",&chN[c]); chIdx[c].resize(tot); for(auto&x:chIdx[c])fscanf(fp,"%d",&x);
    chVal[c].resize(2*chN[c]); for(auto&x:chVal[c]){unsigned b; fscanf(fp,"%x",&b); x=bf(b);} }
  int nq; fscanf(fp,"%d",&nq);
  std::vector<int> qp(nq); std::vector<float> qu(nq), qv(nq);
  for(int i=0;i<nq;i++){unsigned a,b; fscanf(fp,"%d %x %x",&qp[i],&a,&b); qu[i]=bf(a); qv[i]=bf(b);}

  Sdc::Options so; so.SetVtxBoundaryInterpolation((Sdc::Options::VtxBoundaryInterpolation)vb);
  so.SetCreasingMethod(Sdc::Options::CREASE_UNIFORM); so.SetFVarLinearInterpolation((Sdc::Options::FVarLinearInterpolation)fl);
  TopologyDescriptor d; d.numVertices=nv; d.numFaces=nf; d.numVertsPerFace=vpf.data(); d.vertIndicesPerFace=fv.data();
  d.numCreases=nc; d.creaseVertexIndexPairs=cpairs.data(); d.creaseWeights=cw.data();
  d.numCorners=nk; d.cornerVertexIndices=kidx.data(); d.cornerWeights=kw.data();
  std::vector<TopologyDescriptor::FVarChannel> chs(nch);
  for(int c=0;c<nch;c++){chs[c].numValues=chN[c]; chs[c].valueIndices=chIdx[c].data();}
  d.numFVarChannels=nch; d.fvarChannels=chs.data();
  auto*refiner=TopologyRefinerFactory<TopologyDescriptor>::Create(d, TopologyRefinerFactory<TopologyDescriptor>::Options(Sdc::SCHEME_CATMARK,so));
  if(!refiner){fprintf(o,"refiner failed\n");return 1;}
  if(adaptive){ TopologyRefiner::AdaptiveOptions ao(level); ao.considerFVarChannels=nch>0; ao.useInfSharpPatch=true; refiner->RefineAdaptive(ao);}
  else { TopologyRefiner::UniformOptions uo(level); refiner->RefineUniform(uo);}
  fprintf(o,"levels %d\n",refiner->GetNumLevels());
  for(int l=0;l<refiner->GetNumLevels();l++){ auto&L=refiner->GetLevel(l);
    fprintf(o,"%d %d %d",L.GetNumVertices(),L.GetNumEdges(),L.GetNumFaces());
    for(int c=0;c<nch;c++) fprintf(o," %d",L.GetNumFVarValues(c)); fprintf(o,"\n"); }
  std::vector<const StencilTable*> fst(nch);
  for(int c=0;c<nch;c++){ StencilTableFactory::Options op; op.generateOffsets=true; op.generateIntermediateLevels=adaptive;
    op.interpolationMode=StencilTableFactory::INTERPOLATE_FACE_VARYING; op.fvarChannel=c; fst[c]=StencilTableFactory::Create(*refiner,op);
    dumpTable(o,"stencils",fst[c]); }
  PatchTableFactory::Options po(level); po.SetEndCapType(PatchTableFactory::Options::ENDCAP_GREGORY_BASIS); po.useInfSharpPatch=true;
  po.generateFVarTables=nch>0; po.generateFVarLegacyLinearPatches=false;
  const PatchTable* pt=PatchTableFactory::Create(*refiner, po);
  fprintf(o,"patches %d arrays %d\n",pt->GetNumPatchesTotal(),pt->GetNumPatchArrays());
  for(int a=0;a<pt->GetNumPatchArrays();a++){
    for(int p=0;p<pt->GetNumPatches(a);p++){
      PatchTable::PatchHandle h; // not public ctor in all versions; use params by array
      auto pp=pt->GetPatchParams(a)[p];
      fprintf(o,"P a%d f%d u%d v%d d%d nq%d b%d t%d r%d",a,pp.GetFaceId(),pp.GetU(),pp.GetV(),pp.GetDepth(),(int)pp.NonQuadRoot(),pp.GetBoundary(),pp.GetTransition(),(int)pp.IsRegular());
      for(int c=0;c<nch;c++){
        auto fpp=pt->GetPatchArrayFVarPatchParams(a,c)[p];
        auto vals=pt->GetPatchArrayFVarValues(a,c);
        int stride=pt->GetFVarValueStride(c);
        int ncv = fpp.IsRegular()? pt->GetFVarPatchDescriptorRegular(c).GetNumControlVertices(): pt->GetFVarPatchDescriptorIrregular(c).GetNumControlVertices();
        fprintf(o," | b%d r%d :",fpp.GetBoundary(),(int)fpp.IsRegular());
        for(int k=0;k<ncv;k++) fprintf(o," %d",vals[p*stride+k]);
      }
      fprintf(o,"\n");
    }
  }
  std::vector<const StencilTable*> full(nch);
  for(int c=0;c<nch;c++){
    dumpTable(o,"local",pt->GetLocalPointFaceVaryingStencilTable(c));
    const StencilTable* t2=StencilTableFactory::AppendLocalPointStencilTableFaceVarying(*refiner, fst[c], pt->GetLocalPointFaceVaryingStencilTable(c), c);
    full[c]= t2? t2: fst[c];
    dumpTable(o,"appended",t2);
  }
  auto*cpt=Osd::CpuPatchTable::Create(pt);
  PatchMap pm(*pt);
  for(int c=0;c<nch;c++){
    int ncv=full[c]->GetNumControlVertices(), ns=full[c]->GetNumStencils();
    auto*src=Osd::CpuVertexBuffer::Create(2, ncv+ns);
    src->UpdateData(chVal[c].data(),0,chN[c]);
    Osd::BufferDescriptor sd(0,2,2), dd(ncv*2,2,2);
    if(ns>0) Osd::CpuEvaluator::EvalStencils(src,sd,src,dd,full[c]);
    const float*bb=src->BindCpuBuffer();
    fprintf(o,"values %d\n",ncv+ns);
    for(int i=0;i<ncv+ns;i++) fprintf(o,"%08x %08x\n",fb(bb[2*i]),fb(bb[2*i+1]));
    Osd::BufferDescriptor esd(adaptive?0:ncv*2,2,2);
    fprintf(o,"uvs %d\n",nq);
    for(int i=0;i<nq;i++){
      auto*h=pm.FindPatch(qp[i],qu[i],qv[i]);
      Osd::PatchCoord pc(*h,qu[i],qv[i]);
      float r[2]; Osd::BufferDescriptor od(0,2,2);
      Osd::CpuEvaluator::EvalPatches(src->BindCpuBuffer(), esd, r, od, 1, &pc,
         cpt->GetFVarPatchArrayBuffer(c), cpt->GetFVarPatchIndexBuffer(c), cpt->GetFVarPatchParamBuffer(c));
      fprintf(o,"%08x %08x %d\n",fb(r[0]),fb(r[1]),h->patchIndex);
    }
  }
  fclose(o);
  return 0;
}
